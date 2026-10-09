//! The crate's fixers against music21's `alpha.analysis.fixer`.
//!
//! A seeded generator writes a melody in measures and makes two copies of
//! it: a reading, some notes misspelled, their accidentals dropped or
//! naturals written, or read a step off, and a performance, some notes
//! respelled with sharps and some played as the trill -- now and then with
//! a nachschlag -- or the turn they are written as. The two are aligned by
//! music21's `StreamAligner`, the performance as its target, and each
//! fixer is run on fresh copies: `EnharmonicFixer`, `TrillFixer` and
//! `TurnFixer` in place, `TrillFixer` on copies as well, and
//! `DeleteFixer`. Every note of the reading -- its pitch, colour and
//! ornaments -- and the measures left must be music21's.

use music21_rs::alpha::analysis::aligner::StreamAligner;
use music21_rs::alpha::analysis::fixer::{OrnamentFixer, delete_measures, fix_enharmonics};
use music21_rs::expressions::Expression;
use music21_rs::{Duration, Note, Stream, StreamElement, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use music21_rs_python_parity::music21_name;
use pyo3::prelude::*;
use utils::{init_py, prepare};

const ASKED: &str = r#"
from music21 import note, stream
from music21.alpha.analysis import aligner, fixer

def built(measures):
    s = stream.Stream()
    for notes in measures:
        m = stream.Measure()
        for name, length in notes:
            m.append(note.Note(name, quarterLength=length))
        s.append(m)
    return s

def said(s):
    out = []
    for m in s.getElementsByClass(stream.Measure):
        for n in m.notes:
            ornaments = ' '.join(
                f'{type(e).__name__}:{float(e.quarterLength):.6f}:{getattr(e, "nachschlag", False)}'
                for e in n.expressions)
            out.append(f'{n.nameWithOctave}|{n.style.color}|{ornaments}')
        out.append('|')
    return out

def ask(midiMeasures, omrMeasures):
    out = []
    for which in ('enharmonic', 'trill', 'turn', 'trill copied', 'delete'):
        midi = built(midiMeasures)
        omr = built(omrMeasures)
        sa = aligner.StreamAligner(sourceStream=omr, targetStream=midi)
        sa.align()
        try:
            if which == 'enharmonic':
                fixer.EnharmonicFixer(sa.changes, midi, omr).fix()
            elif which == 'trill':
                fixer.TrillFixer(sa.changes, midi, omr).fix()
            elif which == 'turn':
                fixer.TurnFixer(sa.changes, midi, omr).fix()
            elif which == 'trill copied':
                omr = fixer.TrillFixer(sa.changes, midi, omr).fix(inPlace=False).omrStream
            else:
                fixer.DeleteFixer(sa.changes, midi, omr).fix()
        except Exception as error:
            out.append(f'{which}: error {error}')
            continue
        out.append(f'{which}: ' + ' '.join(said(omr)))
    return out
"#;

type Measures = Vec<Vec<(String, f64)>>;

/// A small linear congruential generator, so every run writes the same
/// pairs.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self, below: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % below
    }
}

/// The pitches a melody is written in, each with the spellings a reading
/// or a performance might give it.
const PITCHES: [(&str, &str, &str); 12] = [
    ("C4", "B#3", "Cn4"),
    ("D4", "D4", "Dn4"),
    ("E4", "F-4", "En4"),
    ("F#4", "G-4", "F4"),
    ("G4", "G4", "Gn4"),
    ("G#4", "A-4", "Gn4"),
    ("A4", "A4", "An4"),
    ("B-4", "A#4", "B4"),
    ("B4", "C-5", "Bn4"),
    ("C5", "C5", "Cn5"),
    ("C#5", "D-5", "C5"),
    ("D5", "D5", "Dn5"),
];

/// The note a second above or below, by the generator's pitch list.
fn neighbour(index: usize, up: bool) -> &'static str {
    if up {
        PITCHES[(index + 1).min(PITCHES.len() - 1)].0
    } else {
        PITCHES[index.saturating_sub(1)].0
    }
}

fn pairs() -> Vec<(Measures, Measures)> {
    let mut seeded = Seeded(908);
    let mut out = Vec::new();
    for _ in 0..240 {
        let measure_count = 1 + seeded.next(3) as usize;
        let mut midi: Measures = Vec::new();
        let mut omr: Measures = Vec::new();
        for _ in 0..measure_count {
            let mut played = Vec::new();
            let mut read = Vec::new();
            let mut left = 4.0;
            while left > 0.0 {
                let length = if left >= 2.0 && seeded.next(4) == 0 {
                    2.0
                } else {
                    1.0
                };
                left -= length;
                let index = seeded.next(PITCHES.len() as u64) as usize;
                let (written, respelled, misread) = PITCHES[index];
                // The reading.
                let read_name = match seeded.next(10) {
                    0 => respelled,
                    1 => misread,
                    2 => neighbour(index, seeded.next(2) == 0),
                    _ => written,
                };
                read.push((read_name.to_string(), length));
                // The performance.
                match seeded.next(10) {
                    0 | 1 => {
                        // A trill, up or down, perhaps ending elsewhere.
                        let count = if seeded.next(2) == 0 { 4 } else { 8 };
                        let other = neighbour(index, seeded.next(3) != 0);
                        for k in 0..count {
                            let name = if k % 2 == 0 { written } else { other };
                            played.push((name.to_string(), length / count as f64));
                        }
                        if count == 8 && seeded.next(3) == 0 {
                            let last = played.len() - 1;
                            played[last].0 = neighbour(index, false).to_string();
                        }
                    }
                    2 => {
                        // A turn, from above or below.
                        let from_above = seeded.next(2) == 0;
                        let first = neighbour(index, from_above);
                        let third = neighbour(index, !from_above);
                        for name in [first, written, third, written] {
                            played.push((name.to_string(), length / 4.0));
                        }
                    }
                    3 => played.push((respelled.to_string(), length)),
                    _ => played.push((written.to_string(), length)),
                }
            }
            midi.push(played);
            omr.push(read);
        }
        out.push((midi, omr));
    }
    out
}

fn built(measures: &Measures) -> Stream {
    let mut stream = Stream::new();
    for notes in measures {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        for (name, length) in notes {
            measure.push(
                Note::from_name(name)
                    .expect("a note")
                    .with_duration(Duration::new(*length).expect("a length")),
            );
        }
        stream.push(measure);
    }
    stream
}

fn said(stream: &Stream) -> Vec<String> {
    let mut out = Vec::new();
    for event in stream.events() {
        let StreamElement::Stream(measure) = event.element() else {
            continue;
        };
        for inner in measure.events() {
            let StreamElement::Note(note) = inner.element() else {
                continue;
            };
            let ornaments: Vec<String> = note
                .expressions()
                .iter()
                .filter_map(|expression| match expression {
                    Expression::Ornament(ornament) => Some(format!(
                        "{}:{:.6}:{}",
                        ornament.kind().class_name(),
                        ornament.quarter_length(),
                        if ornament.nachschlag() {
                            "True"
                        } else {
                            "False"
                        }
                    )),
                    _ => None,
                })
                .collect();
            out.push(format!(
                "{}|{}|{}",
                music21_name(&note.pitch().name_with_octave()),
                note.color().unwrap_or("None"),
                ornaments.join(" ")
            ));
        }
        out.push("|".to_string());
    }
    out
}

fn ours(midi_measures: &Measures, omr_measures: &Measures) -> Vec<String> {
    let mut out = Vec::new();
    for which in ["enharmonic", "trill", "turn", "trill copied", "delete"] {
        let midi = built(midi_measures);
        let mut omr = built(omr_measures);
        let mut aligner = StreamAligner::new();
        let result = aligner.align(&midi, &omr).and_then(|()| {
            let changes = &aligner.changes;
            match which {
                "enharmonic" => fix_enharmonics(changes, &midi, &mut omr),
                "trill" => OrnamentFixer::trills().fix(changes, &midi, &mut omr, false),
                "turn" => OrnamentFixer::turns().fix(changes, &midi, &mut omr, false),
                "trill copied" => {
                    omr = OrnamentFixer::trills().fixed(&midi, &omr)?;
                    Ok(())
                }
                _ => delete_measures(changes, &midi, &mut omr),
            }
        });
        out.push(match result {
            Ok(()) => format!("{which}: {}", said(&omr).join(" ")),
            Err(error) => format!(
                "{which}: error {}",
                error.to_string().trim_start_matches("Analysis error: ")
            ),
        });
    }
    out
}

#[test]
fn readings_are_fixed_as_music21_fixes_them() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");
    let pairs = pairs();

    let failures = Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let module = PyModule::from_code(
            py,
            &std::ffi::CString::new(ASKED).expect("no nul in the helper"),
            c"fixer_parity.py",
            c"fixer_parity",
        )?;
        let ask = module.getattr("ask")?;
        let mut failures = Vec::new();
        let mut counts = [0usize; 4];
        for (midi, omr) in &pairs {
            let theirs: Vec<String> = ask.call1((midi.clone(), omr.clone()))?.extract()?;
            counts[0] += usize::from(theirs[1].contains("Trill:"));
            counts[1] += usize::from(theirs[2].contains("Turn:"));
            counts[2] += theirs.iter().filter(|answer| answer.contains("error")).count();
            counts[3] += usize::from(
                theirs[4].matches('|').count() < theirs[0].matches('|').count(),
            );
            let ours = ours(midi, omr);
            for (ours, theirs) in ours.iter().zip(&theirs) {
                if ours != theirs {
                    failures.push(format!(
                        "midi {midi:?}\nomr  {omr:?}\n  music21    {theirs}\n  music21-rs {ours}"
                    ));
                }
            }
        }
        println!(
            "music21 wrote trills on {} readings and turns on {}, refused {} times, and deleted measures from {}",
            counts[0], counts[1], counts[2], counts[3]
        );
        Ok(failures)
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    println!("{} pairs compared", pairs.len());
    assert!(
        failures.is_empty(),
        "{} answers differ:\n\n{}",
        failures.len(),
        failures
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}
