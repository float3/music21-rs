//! The wheel's `Stream.placed()` against what music21 answers through an
//! element's sites.
//!
//! music21 hangs `offset`, `beat`, `beatStr`, `beatDuration`,
//! `beatStrength` and `measureNumber` on every element and works each out
//! from the streams holding it. The package keeps no sites, so its streams
//! answer all six for every element at once; each music below is built
//! twice, once of music21's classes and once of the package's, and every
//! element has to come out the same.

use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use music21_rs_python_parity::music21_rs_facade;
use pyo3::prelude::*;
use utils::{init_py, prepare};

/// Builds each case of both libraries and reads it: music21's through its
/// sites, walking `recurse` as `getOffsetInHierarchy` counts, the
/// package's through `placed`.
const CASES: &str = r#"
import types

from music21 import bar, meter, note, stream
import music21_rs_facade as rs

M21 = types.SimpleNamespace(
    Part=stream.Part,
    Measure=stream.Measure,
    Score=stream.Score,
    Note=note.Note,
    TimeSignature=meter.TimeSignature,
)

CASES = ['three_four_without_measures', 'pickup_with_padding', 'two_parts_changing_meter']


def notes(lib, held, lengths, name='C4'):
    for length in lengths:
        held.append(lib.Note(name, quarterLength=length))


def measure(lib, number, meter_text, lengths, padding=0.0):
    made = lib.Measure(number=number)
    if meter_text:
        made.insert(0, lib.TimeSignature(meter_text))
    if padding:
        made.paddingLeft = padding
    notes(lib, made, lengths)
    return made


def build(lib, case):
    if case == 'three_four_without_measures':
        part = lib.Part()
        part.append(lib.TimeSignature('3/4'))
        notes(lib, part, [1, 0.5, 0.5, 2, 1, 1.5, 0.5, 1, 1])
        return part
    if case == 'pickup_with_padding':
        part = lib.Part()
        part.append(measure(lib, 0, '4/4', [1], padding=3.0))
        part.append(measure(lib, 1, None, [1, 1, 0.5, 0.5, 1]))
        part.append(measure(lib, 2, None, [2, 2]))
        return part
    score = lib.Score()
    for lengths in ([1, 1, 1, 1], [2, 2]):
        part = lib.Part()
        part.append(measure(lib, 1, '4/4', lengths))
        part.append(measure(lib, 2, '6/8', [1.5, 0.5, 0.5, 0.5]))
        part.append(measure(lib, 3, None, [0.5, 1, 1.5]))
        score.insert(0, part)
    return score


def row(element, offset, measure_number, beat, beat_str, beat_duration, beat_strength):
    return (
        type(element).__name__,
        float(offset),
        measure_number,
        float(beat),
        beat_str,
        float(beat_duration.quarterLength),
        float(beat_strength),
    )


def theirs(case):
    built = build(M21, case)
    rows = []
    for element in built.recurse():
        if element.isStream or isinstance(element, bar.Barline):
            continue
        rows.append(row(
            element,
            element.getOffsetInHierarchy(built),
            element.measureNumber,
            element.beat,
            element.beatStr,
            element.beatDuration,
            element.beatStrength,
        ))
    return rows


def ours(case):
    return [
        row(
            each.element,
            each.offset,
            each.measureNumber,
            each.beat,
            each.beatStr,
            each.beatDuration,
            each.beatStrength,
        )
        for each in build(rs, case).placed()
    ]
"#;

/// One element as both libraries describe it.
type Row = (String, f64, Option<i32>, f64, String, f64, f64);

fn same(left: &Row, right: &Row) -> bool {
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9 || (a.is_nan() && b.is_nan());
    left.0 == right.0
        && close(left.1, right.1)
        && left.2 == right.2
        && close(left.3, right.3)
        && left.4 == right.4
        && close(left.5, right.5)
        && close(left.6, right.6)
}

#[test]
fn placed_answers_what_music21s_sites_answer() {
    pyo3::append_to_inittab!(music21_rs_facade);
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let failures = Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let cases = PyModule::from_code(
            py,
            &std::ffi::CString::new(CASES).expect("no nul in the cases"),
            c"placed_parity_cases.py",
            c"placed_parity_cases",
        )?;
        let names: Vec<String> = cases.getattr("CASES")?.extract()?;
        let mut failures = Vec::new();
        for name in &names {
            let theirs: Vec<Row> = cases.getattr("theirs")?.call1((name,))?.extract()?;
            let ours: Vec<Row> = cases.getattr("ours")?.call1((name,))?.extract()?;
            if theirs.is_empty() {
                failures.push(format!("{name}: music21 places nothing"));
                continue;
            }
            if theirs.len() != ours.len() {
                failures.push(format!(
                    "{name}: music21 places {} elements, the package {}",
                    theirs.len(),
                    ours.len()
                ));
                continue;
            }
            for (at, (theirs, ours)) in theirs.iter().zip(&ours).enumerate() {
                if !same(theirs, ours) {
                    failures.push(format!("{name} [{at}]: music21 {theirs:?}, ours {ours:?}"));
                }
            }
        }
        Ok(failures)
    })
    .unwrap_or_else(|error| panic!("running the cases: {error}"));

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
