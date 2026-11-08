//! The wheel's `Stream.placed()` against what music21 answers through an
//! element's sites.
//!
//! music21 hangs `offset`, `beat`, `beatStr`, `beatDuration`,
//! `beatStrength` and `measureNumber` on every element and works each out
//! from the streams holding it. The package's streams also answer all six
//! for every element at once, from the crate; each music below is built
//! twice, once of music21's classes and once of the package's, and every
//! element has to come out the same. Beside the cases written out, a
//! thousand pieces are made up from seeds.

use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use music21_rs_python_parity::music21_rs_facade;
use pyo3::prelude::*;
use utils::{init_py, prepare};

/// Builds each case of both libraries and reads it: music21's through its
/// sites, walking `recurse` as `getOffsetInHierarchy` counts, the
/// package's through `placed`.
const CASES: &str = r#"
import random
import types
from fractions import Fraction

from music21 import bar, chord, meter, note, stream
import music21_rs_facade as rs

M21 = types.SimpleNamespace(
    Part=stream.Part,
    Measure=stream.Measure,
    Score=stream.Score,
    Note=note.Note,
    Rest=note.Rest,
    Chord=chord.Chord,
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


METERS = ['4/4', '3/4', '2/4', '6/8', '3/8', '5/8', '2/2', '12/8', '7/8', '3/2', '9/8', '1/4', '5/4']
LENGTHS = [
    Fraction(1, 4), Fraction(1, 2), Fraction(3, 4), 1, Fraction(3, 2), 2, 3,
    Fraction(1, 3), Fraction(2, 3), Fraction(1, 6),
]
PADDINGS = [Fraction(1, 2), 1, 2, 3, Fraction(3, 2), Fraction(1, 3)]


def plan(seed):
    """One or two parts made up from a seed: measures numbered from 0 to 2
    on, the first often a pickup, meters changing now and then, measures
    under- or overfull or empty; or now and then notes with no measures."""
    r = random.Random(seed)
    parts = []
    measured = r.random() < 0.8
    for _ in range(1 + (r.random() < 0.3)):
        if not measured:
            events = [('ts', r.choice(METERS))] if r.random() < 0.8 else []
            for _ in range(r.randint(1, 10)):
                if r.random() < 0.1:
                    events.append(('ts', r.choice(METERS)))
                events.append((r.choice('nnnrc'), r.choice(LENGTHS)))
            parts.append(('flat', events))
            continue
        measures = []
        first = r.randint(0, 2)
        for number in range(first, first + r.randint(1, 4)):
            starts = number == first and r.random() < 0.9
            meter_text = r.choice(METERS) if starts or r.random() < 0.25 else None
            padding = 0
            if number == first and r.random() < 0.4:
                padding = r.choice(PADDINGS)
            lengths = [(r.choice('nnnrc'), r.choice(LENGTHS)) for _ in range(r.randint(0, 6))]
            measures.append((number, meter_text, padding, lengths))
        parts.append(('measured', measures))
    return parts


def event(lib, kind, length):
    if kind == 'r':
        return lib.Rest(quarterLength=length)
    if kind == 'c':
        return lib.Chord(['C4', 'E4'], quarterLength=length)
    return lib.Note('C4', quarterLength=length)


def generated(lib, seed):
    made = []
    for kind, content in plan(seed):
        part = lib.Part()
        if kind == 'flat':
            for what, value in content:
                part.append(lib.TimeSignature(value) if what == 'ts' else event(lib, what, value))
        else:
            for number, meter_text, padding, lengths in content:
                held = lib.Measure(number=number)
                if meter_text:
                    held.insert(0, lib.TimeSignature(meter_text))
                if padding:
                    held.paddingLeft = padding
                for what, length in lengths:
                    held.append(event(lib, what, length))
                part.append(held)
        made.append(part)
    if len(made) == 1:
        return made[0]
    score = lib.Score()
    for part in made:
        score.insert(0, part)
    return score


def build(lib, case):
    if isinstance(case, int):
        return generated(lib, case)
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

/// How many pieces the seeded test makes up: about seven seconds of
/// music21 working out its sites.
const GENERATED: u64 = 1000;

/// Compares one case, a fixed name or a seed, row by row.
fn compare(
    cases: &Bound<'_, PyModule>,
    case: &Bound<'_, PyAny>,
    label: &str,
    failures: &mut Vec<String>,
) -> PyResult<()> {
    let theirs: Vec<Row> = cases.getattr("theirs")?.call1((case,))?.extract()?;
    let ours: Vec<Row> = match cases.getattr("ours")?.call1((case,)) {
        Ok(rows) => rows.extract()?,
        Err(error) => {
            failures.push(format!("{label}: the package raised {error}"));
            return Ok(());
        }
    };
    if theirs.is_empty() {
        return Ok(());
    }
    if theirs.len() != ours.len() {
        failures.push(format!(
            "{label}: music21 places {} elements, the package {}",
            theirs.len(),
            ours.len()
        ));
        return Ok(());
    }
    for (at, (theirs, ours)) in theirs.iter().zip(&ours).enumerate() {
        if !same(theirs, ours) {
            failures.push(format!("{label} [{at}]: music21 {theirs:?}, ours {ours:?}"));
        }
    }
    Ok(())
}

/// Runs `check` with the cases module loaded beside music21 and the
/// package, and returns what it found wrong.
fn with_cases(
    check: impl FnOnce(&Bound<'_, PyModule>, &mut Vec<String>) -> PyResult<()>,
) -> Vec<String> {
    // The two tests share one interpreter, which takes the module only
    // before it starts.
    static READY: std::sync::Once = std::sync::Once::new();
    let root = repo_root();
    READY.call_once(|| {
        pyo3::append_to_inittab!(music21_rs_facade);
        std::env::set_current_dir(&root).expect("chdir to the repository root");
        prepare().expect("prepare the music21 reference checkout");
    });

    Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let cases = PyModule::from_code(
            py,
            &std::ffi::CString::new(CASES).expect("no nul in the cases"),
            c"placed_parity_cases.py",
            c"placed_parity_cases",
        )?;
        let mut failures = Vec::new();
        check(&cases, &mut failures)?;
        Ok(failures)
    })
    .unwrap_or_else(|error| panic!("running the cases: {error}"))
}

#[test]
fn placed_answers_what_music21s_sites_answer() {
    let failures = with_cases(|cases, failures| {
        let names: Vec<String> = cases.getattr("CASES")?.extract()?;
        for name in &names {
            let theirs = cases.getattr("theirs")?.call1((name,))?.len()?;
            if theirs == 0 {
                failures.push(format!("{name}: music21 places nothing"));
                continue;
            }
            compare(
                cases,
                &name.into_pyobject(cases.py())?.into_any(),
                name,
                failures,
            )?;
        }
        Ok(())
    });

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Pieces made up from seeds 0 to [`GENERATED`]: pickups, padding, meters
/// changing and measures over- or underfull or empty. A failure names its
/// seed, which `plan` in the cases turns back into the piece.
#[test]
fn placed_answers_alike_on_generated_pieces() {
    let failures = with_cases(|cases, failures| {
        for seed in 0..GENERATED {
            let case = seed.into_pyobject(cases.py())?.into_any();
            compare(cases, &case, &format!("seed {seed}"), failures)?;
        }
        Ok(())
    });

    assert!(
        failures.is_empty(),
        "{} rows differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
