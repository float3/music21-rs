//! The crate's fretboards against music21's: `tablature`.
//!
//! Every board music21 has -- a bare one, a guitar, a ukulele, a bass guitar
//! and a mandolin -- holds a note on each string from nought to past its
//! last, at each fret up to two octaves and each finger: the two must write
//! the same notes, order them the same lowest first, and sound the same
//! pitches or refuse the same boards.

use music21_rs::IntegerType;
use music21_rs::tablature::{FretBoard, FretNote};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

const MUSIC21: &str = r#"
from music21 import tablature

BOARDS = [lambda notes: tablature.FretBoard(6, fretNotes=notes),
          lambda notes: tablature.GuitarFretBoard(fretNotes=notes),
          lambda notes: tablature.UkeleleFretBoard(fretNotes=notes),
          lambda notes: tablature.BassGuitarFretBoard(fretNotes=notes),
          lambda notes: tablature.MandolinFretBoard(fretNotes=notes)]

def note_text(note):
    text = repr(note)
    prefix = '<music21.tablature.FretNote '
    return text[len(prefix):-1] if text.startswith(prefix) else text[len(prefix) - 1:-1]

def report(cases):
    out = []
    for board, notes in cases:
        fret_notes = [tablature.FretNote(string, fret, finger) for string, fret, finger in notes]
        b = BOARDS[board](fret_notes)
        texts = [note_text(n) for n in fret_notes]
        lowest = [n.string for n in b.fretNotesLowestFirst()]
        try:
            pitches = [None if p is None else (float(p.ps), p.nameWithOctave) for p in b.getPitches()]
            error = ''
        except Exception as e:
            pitches = []
            error = type(e).__name__
        out.append((texts, lowest, pitches, error))
    return out
"#;

type Note = (
    Option<IntegerType>,
    Option<IntegerType>,
    Option<IntegerType>,
);
type Answer = (
    Vec<String>,
    Vec<Option<IntegerType>>,
    Vec<Option<(f64, String)>>,
    String,
);

/// A pitch's name as music21 writes it, a flat as `-`.
fn music21_name(pitch: &music21_rs::pitch::Pitch) -> String {
    let name = pitch.name_with_octave();
    let mut letters = name.chars();
    let step = letters.next().map(String::from).unwrap_or_default();
    step + &letters.as_str().replace('b', "-")
}

fn board(kind: usize, notes: Vec<FretNote>) -> FretBoard {
    match kind {
        0 => FretBoard::new(6, notes, 4),
        1 => FretBoard::guitar(notes, 4),
        2 => FretBoard::ukulele(notes, 4),
        3 => FretBoard::bass_guitar(notes, 4),
        _ => FretBoard::mandolin(notes, 4),
    }
}

#[test]
fn the_crate_reads_fretboards_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let mut cases: Vec<(usize, Vec<Note>)> = Vec::new();
    for kind in 0..5 {
        for string in -1..=8 {
            for fret in [0, 1, 2, 5, 11, 12, 13, 24] {
                cases.push((kind, vec![(Some(string), Some(fret), Some(fret % 5))]));
            }
        }
        cases.push((kind, vec![(None, None, None)]));
        cases.push((kind, vec![(None, Some(3), None), (Some(2), None, Some(1))]));
        cases.push((
            kind,
            vec![
                (Some(3), Some(2), None),
                (Some(2), Some(2), Some(3)),
                (Some(4), Some(2), None),
                (Some(5), Some(0), None),
                (Some(2), Some(7), None),
            ],
        ));
        cases.push((kind, Vec::new()));
    }

    let failures = Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"tablature_parity_music21.py",
            c"tablature_parity_music21",
        )?;
        let theirs: Vec<Answer> = music21
            .getattr("report")?
            .call1((cases.clone(),))?
            .extract()?;
        let mut failures = Vec::new();
        for ((kind, notes), theirs) in cases.iter().zip(theirs) {
            let fret_notes: Vec<FretNote> = notes
                .iter()
                .map(|(string, fret, finger)| FretNote::new(*string, *fret, *finger))
                .collect();
            let texts: Vec<String> = fret_notes.iter().map(ToString::to_string).collect();
            let board = board(*kind, fret_notes);
            let lowest: Vec<Option<IntegerType>> = board
                .fret_notes_lowest_first()
                .iter()
                .map(|note| note.string)
                .collect();
            let (pitches, error) = match board.pitches() {
                Ok(pitches) => (
                    pitches
                        .iter()
                        .map(|pitch| pitch.as_ref().map(|p| (p.ps(), music21_name(p))))
                        .collect(),
                    String::new(),
                ),
                Err(error) => (Vec::new(), error.to_string()),
            };
            let pitches_agree = if theirs.3.is_empty() {
                error.is_empty() && pitches == theirs.2
            } else {
                !error.is_empty()
            };
            if texts != theirs.0 || lowest != theirs.1 || !pitches_agree {
                failures.push(format!(
                    "board {kind} {notes:?}:\n  music21    {theirs:?}\n  music21-rs {:?}",
                    (texts, lowest, pitches, error)
                ));
            }
        }
        Ok(failures)
    })
    .expect("the Python side runs");

    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n")
    );
}
