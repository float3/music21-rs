//! The crate's ornament recognizers against music21's
//! `alpha.analysis.ornamentRecognizer`.
//!
//! Runs of notes are built from a pool of pitches spelled every way the
//! recognizers tell apart: notes alternating between two pitches, with and
//! without a tail leaving the alternation, and four notes turning about a
//! middle one, a few with a rest or a chord among them, each with no simple
//! note, a simple note on either pitch or on neither, and a rest for one.
//! Each run is recognized by music21's `TrillRecognizer`, with and without
//! `checkNachschlag`, and its `TurnRecognizer`, and the crate's must answer
//! the same: no ornament, or the same class with the same note length,
//! nachschlag, accidental and size, or the same error.

use music21_rs::alpha::analysis::ornament_recognizer::{TrillRecognizer, TurnRecognizer};
use music21_rs::expressions::Ornament;
use music21_rs::{Chord, Duration, KeySignature, Note, Rest, StreamElement};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use music21_rs_python_parity::music21_name;
use pyo3::prelude::*;
use utils::{init_py, prepare};

const ASKED: &str = r#"
from music21 import chord, note
from music21.alpha.analysis import ornamentRecognizer

def made(spec):
    kind, name, length = spec
    if kind == 'rest':
        return note.Rest(quarterLength=length)
    if kind == 'chord':
        return chord.Chord(name.split(), quarterLength=length)
    return note.Note(name, quarterLength=length)

def said(found, busy, simple):
    if found is False:
        return 'False'
    accidental = found.accidental.name if getattr(found, 'accidental', None) is not None else None
    text = (f'{type(found).__name__} ql={float(found.quarterLength):.6f} '
            f'nachschlag={getattr(found, "nachschlag", False)} accidental={accidental}')
    if hasattr(found, 'nachschlag'):
        start = busy[0]
        if simple and simple[0].pitch.midi == busy[1].pitch.midi:
            start = busy[1]
        text += f' size={found.getSize(start).directedName}'
    return text

def ask(busySpecs, simpleSpecs):
    out = []
    for which in ('trill', 'nachschlag', 'turn'):
        busy = [made(spec) for spec in busySpecs]
        simple = [made(spec) for spec in simpleSpecs]
        if which == 'turn':
            recognizer = ornamentRecognizer.TurnRecognizer()
        else:
            recognizer = ornamentRecognizer.TrillRecognizer(checkNachschlag=(which == 'nachschlag'))
        try:
            found = recognizer.recognize(busy, simpleNotes=simple or None)
        except Exception as error:
            out.append(f'error: {error}')
            continue
        out.append(said(found, busy, simple))
    return out
"#;

/// A note, chord or rest to build: its kind, its pitch names, its length.
type Spec = (&'static str, String, f64);

fn made(spec: &Spec) -> StreamElement {
    let (kind, name, length) = spec;
    let duration = Duration::new(*length).expect("a length");
    match *kind {
        "rest" => StreamElement::Rest(Rest::new(duration)),
        "chord" => StreamElement::Chord(
            Chord::new(name.as_str())
                .expect("a chord")
                .with_duration(duration),
        ),
        _ => StreamElement::Note(
            Note::from_name(name)
                .expect("a note")
                .with_duration(duration),
        ),
    }
}

fn said(found: Option<Ornament>, busy: &[StreamElement], simple: &[StreamElement]) -> String {
    let Some(found) = found else {
        return "False".to_string();
    };
    let class = found.kind().class_name();
    let accidental = found.accidental().map_or_else(
        || "None".to_string(),
        |accidental| accidental.name().to_string(),
    );
    let python_bool = if found.nachschlag() { "True" } else { "False" };
    let mut text = format!(
        "{class} ql={:.6} nachschlag={python_bool} accidental={accidental}",
        found.quarter_length()
    );
    if found.is_a("Trill") {
        let pitch = |element: &StreamElement| match element {
            StreamElement::Note(note) => note.pitch().clone(),
            _ => unreachable!("a trill is recognized from notes"),
        };
        let mut start = pitch(&busy[0]);
        if let Some(StreamElement::Note(written)) = simple.first()
            && written.pitch().midi() == pitch(&busy[1]).midi()
        {
            start = pitch(&busy[1]);
        }
        let size = found
            .size(&start, &KeySignature::new(0))
            .map(|size| size.directed_name())
            .unwrap_or_else(|error| format!("error {error}"));
        text.push_str(&format!(" size={size}"));
    }
    text
}

fn ours(busy_specs: &[Spec], simple_specs: &[Spec]) -> Vec<String> {
    let busy: Vec<StreamElement> = busy_specs.iter().map(made).collect();
    let simple: Vec<StreamElement> = simple_specs.iter().map(made).collect();
    let mut out = Vec::new();
    for which in ["trill", "nachschlag", "turn"] {
        let found = match which {
            "turn" => TurnRecognizer::default().recognize(&busy, &simple),
            _ => TrillRecognizer {
                check_nachschlag: which == "nachschlag",
                ..TrillRecognizer::default()
            }
            .recognize(&busy, &simple),
        };
        out.push(match found {
            Ok(found) => said(found, &busy, &simple),
            Err(error) => format!(
                "error: {}",
                error.to_string().trim_start_matches("Analysis error: ")
            ),
        });
    }
    out
}

const POOL: [&str; 14] = [
    "E4", "F4", "F#4", "G-4", "G4", "G#4", "A-4", "A4", "A#4", "B-4", "B4", "C5", "G##4", "D5",
];

fn note(name: &str, length: f64) -> Spec {
    ("note", name.to_string(), length)
}

/// Every run of busy notes, each with the simple notes it is tried with.
fn cases() -> Vec<(Vec<Spec>, Vec<Spec>)> {
    let mut runs: Vec<Vec<Spec>> = Vec::new();
    for (i, a) in POOL.iter().enumerate() {
        for (j, b) in POOL.iter().enumerate() {
            if i == j {
                continue;
            }
            for count in [3, 4, 5, 8] {
                let length = 1.0 / count as f64;
                let alternating: Vec<Spec> = (0..count)
                    .map(|k| note(if k % 2 == 0 { a } else { b }, length))
                    .collect();
                runs.push(alternating.clone());
                if count >= 5 {
                    // A tail leaving the alternation, two or three notes long.
                    for tail in [2, 3] {
                        let mut tailed = alternating.clone();
                        for (place, name) in tailed[count - tail..]
                            .iter_mut()
                            .zip(POOL[(j + 1) % POOL.len()..].iter().chain(POOL.iter()))
                        {
                            place.1 = (*name).to_string();
                        }
                        runs.push(tailed);
                    }
                }
            }
        }
    }
    let middles = ["F#4", "G4", "G#4", "A-4", "A4", "B-4", "C5"];
    for upper in POOL {
        for middle in middles {
            for lower in POOL {
                runs.push(vec![
                    note(upper, 0.25),
                    note(middle, 0.25),
                    note(lower, 0.25),
                    note(middle, 0.25),
                ]);
            }
        }
    }
    // Lengths that differ, and runs with a rest or a chord among the notes.
    let rubato = [0.25, 0.15, 0.2, 0.4];
    runs.push(
        ["G4", "F#4", "E4", "F#4"]
            .iter()
            .zip(rubato)
            .map(|(name, length)| note(name, length))
            .collect(),
    );
    for odd in [("rest", String::new()), ("chord", "E4 G4".to_string())] {
        for at in 0..4 {
            for base in [["G4", "A4", "G4", "A4"], ["G4", "F#4", "E4", "F#4"]] {
                let mut run: Vec<Spec> = base.iter().map(|name| note(name, 0.25)).collect();
                run[at] = (odd.0, odd.1.clone(), 0.25);
                runs.push(run);
            }
        }
    }

    let mut out = Vec::new();
    for run in runs {
        let mut simples: Vec<Vec<Spec>> = vec![Vec::new()];
        if let (Some(first), Some(second)) = (run.first(), run.get(1)) {
            for name in [&first.1, &second.1, &"D4".to_string()] {
                if !name.is_empty() && !name.contains(' ') {
                    simples.push(vec![note(name, 1.0)]);
                }
            }
        }
        simples.push(vec![("rest", String::new(), 1.0)]);
        for simple in simples {
            out.push((run.clone(), simple));
        }
    }
    out
}

#[test]
fn ornaments_are_recognized_as_music21_recognizes_them() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");
    let cases = cases();

    let failures = Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let asked = PyModule::from_code(
            py,
            &std::ffi::CString::new(ASKED).expect("no nul in the helper"),
            c"ornament_recognizer.py",
            c"ornament_recognizer",
        )?;
        let ask = asked.getattr("ask")?;
        let mut failures = Vec::new();
        for (busy, simple) in &cases {
            let theirs: Vec<String> = ask.call1((busy.clone(), simple.clone()))?.extract()?;
            let ours = ours(busy, simple);
            if ours != theirs {
                let names: Vec<String> = busy
                    .iter()
                    .map(|(kind, name, length)| format!("{kind} {} {length}", music21_name(name)))
                    .collect();
                failures.push(format!(
                    "{names:?} simple {simple:?}:\n  music21    {theirs:?}\n  music21-rs {ours:?}"
                ));
            }
        }
        Ok(failures)
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    println!("{} runs compared", cases.len());
    assert!(
        failures.is_empty(),
        "{} of {} runs differ:\n\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .take(30)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}
