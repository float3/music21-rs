//! The scores the crate's figured-bass realizer writes against music21's:
//! `FiguredBassLine.generateBassLine` and `overlayPart`, and
//! `Realization.generateRealizationFromPossibilityProgression`,
//! `generateAllRealizations`, `generateRandomRealization` and
//! `generateRandomRealizations`.
//!
//! Each line is built on both sides and realized on both. The two must find
//! the same voicings in the same order, and every score either writes is
//! put through its MusicXML writer -- music21's own exporter, and the
//! crate's `to_musicxml`, which `musicxml_parity` holds to it -- and the
//! two documents must be the same text. Where music21 chooses at random its
//! `random.sample` is replaced by a choice the crate is given too.
//!
//! Each score is asked of a realization made afresh, so that what one
//! realization chose cannot carry into the next.
//!
//! music21 is music21 here, with nothing of the crate installed over it. A
//! mismatch leaves both documents in `target/realizer-parity/`.

use music21_rs::figuredbass::realizer::{FiguredBassLine, Realization};
use music21_rs::figuredbass::rules::Rules;
use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs::{
    Duration, Key, Note, Pitch, Rest, RomanNumeral, Stream, StreamKind, TimeSignature,
};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use music21_rs_python_parity::music21_name;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{STRIP_LAYOUT, first_difference, normalize_ids};

/// A line: its key, its meter, its bass notes with their lengths and
/// figures -- or the roman numerals it is a line of chords of -- and the
/// parts laid over it, a rest being a note with no name.
struct Line {
    label: &'static str,
    key: &'static str,
    meter: &'static str,
    notes: &'static [(&'static str, f64, Option<&'static str>)],
    chords: &'static [&'static str],
    overlaid: &'static [&'static [(Option<&'static str>, f64)]],
}

const LINES: &[Line] = &[
    Line {
        label: "a note tied over a barline, figures on two lines",
        key: "B",
        meter: "3/4",
        notes: &[
            ("B2", 1.0, None),
            ("C#3", 1.0, Some("6")),
            ("D#3", 2.5, Some("6,#4")),
            ("E3", 0.5, None),
        ],
        chords: &[],
        overlaid: &[],
    },
    Line {
        label: "a suspension",
        key: "C",
        meter: "4/4",
        notes: &[
            ("C3", 1.0, None),
            ("D3", 1.0, Some("4,3")),
            ("C3", 2.0, None),
        ],
        chords: &[],
        overlaid: &[],
    },
    Line {
        label: "a dominant seventh resolving, few enough ways to write them all",
        key: "C",
        meter: "2/4",
        notes: &[("G2", 1.0, Some("7")), ("C3", 1.0, None)],
        chords: &[],
        overlaid: &[],
    },
    Line {
        label: "a minor key, eighths to beam, an accidental to write",
        key: "d",
        meter: "3/4",
        notes: &[
            ("D3", 0.5, None),
            ("E3", 0.5, Some("6")),
            ("F3", 0.5, Some("6")),
            ("G3", 0.5, None),
            ("A2", 1.0, Some("#")),
            ("D3", 3.0, None),
        ],
        chords: &[],
        overlaid: &[],
    },
    Line {
        label: "a triplet, bracketed and beamed",
        key: "C",
        meter: "2/4",
        notes: &[
            ("C3", 1.0 / 3.0, None),
            ("D3", 1.0 / 3.0, Some("6")),
            ("E3", 1.0 / 3.0, Some("6")),
            ("F3", 1.0, Some("6,4")),
            ("C3", 2.0, None),
        ],
        chords: &[],
        overlaid: &[],
    },
    Line {
        label: "one chord",
        key: "C",
        meter: "4/4",
        notes: &[("C3", 4.0, None)],
        chords: &[],
        overlaid: &[],
    },
    Line {
        label: "a melody laid over the line, moving once while the bass holds",
        key: "C",
        meter: "4/4",
        notes: &[
            ("C3", 1.0, None),
            ("D3", 1.0, Some("6")),
            ("E3", 2.0, Some("6")),
        ],
        chords: &[],
        overlaid: &[&[
            (Some("E5"), 1.0),
            (Some("F5"), 1.0),
            (Some("G5"), 1.0),
            (Some("C5"), 1.0),
        ]],
    },
    Line {
        label: "two parts laid over the line",
        key: "C",
        meter: "4/4",
        notes: &[("C3", 2.0, None), ("G2", 2.0, None), ("C3", 4.0, None)],
        chords: &[],
        overlaid: &[
            &[(Some("E5"), 2.0), (Some("D5"), 2.0), (Some("C5"), 4.0)],
            &[(Some("G4"), 2.0), (Some("B4"), 2.0), (Some("G4"), 4.0)],
        ],
    },
    Line {
        label: "a line of chords, as music21 reads roman numerals into one",
        key: "C",
        meter: "2/4",
        notes: &[],
        chords: &["I", "IV", "V7", "I"],
        overlaid: &[],
    },
    Line {
        label: "a compound meter in a flat key",
        key: "g",
        meter: "6/8",
        notes: &[
            ("G2", 1.5, None),
            ("A2", 0.5, Some("6")),
            ("B-2", 0.5, Some("6")),
            ("C3", 0.5, Some("6")),
            ("D3", 1.5, Some("#")),
            ("D3", 1.5, Some("7,#")),
            ("G2", 3.0, None),
        ],
        chords: &[],
        overlaid: &[],
    },
];

/// What music21 is asked, and the choice put in the place of its
/// `random.sample`.
const ASKED: &str = r#"
from music21 import key, meter, note, pitch, roman, stream
from music21.figuredBass import realizer

def line(tonic, ratio, notes, chords, overlaid):
    made = realizer.FiguredBassLine(key.Key(tonic), meter.TimeSignature(ratio))
    for name, length, notation in notes:
        made.addElement(note.Note(name, quarterLength=length), notation)
    for figure in chords:
        made.addElement(roman.RomanNumeral(figure, tonic))
    for part in overlaid:
        held = stream.Part()
        for name, length in part:
            if name is None:
                held.append(note.Rest(quarterLength=length))
            else:
                held.append(note.Note(name, quarterLength=length))
        made.overlayPart(held)
    return made

def in_score(part):
    score = stream.Score()
    score.insert(0, part)
    return score

def names(progressions):
    return [[[p.nameWithOctave for p in voicing] for voicing in progression]
            for progression in progressions]

def segments(realization):
    return [f'{each.bassNote.nameWithOctave} {float(each.quarterLength):.5f}'
            for each in realization._segmentList]

class Chooser:
    """Stands in for the `random` module: the n-th choice is the same on both sides."""
    def __init__(self):
        self.calls = 0

    def sample(self, population, count):
        index = (self.calls * 31 + 7) % len(population)
        self.calls += 1
        return [population[index]]

def choose_as_the_crate_does():
    realizer.random = Chooser()

def way(realization, index):
    progression = realization.getAllPossibilityProgressions()[index]
    return realization.generateRealizationFromPossibilityProgression(progression)

def chosen_way(realization):
    choose_as_the_crate_does()
    progression = realization.getRandomPossibilityProgression()
    return realization.generateRealizationFromPossibilityProgression(progression)
"#;

fn our_line(line: &Line) -> FiguredBassLine {
    let key = Key::from_tonic(line.key).expect("a key");
    let meter = TimeSignature::from_ratio_string(line.meter).expect("a meter");
    let mut made = FiguredBassLine::in_key_and_time(&key, meter).expect("a line");
    for (name, length, notation) in line.notes {
        made.add_element(Pitch::from_name(name).expect("a pitch"), *length, *notation);
    }
    for figure in line.chords {
        // music21 hands the realizer the numeral's bass, the names of its
        // notes with none said twice, and its length, a quarter.
        let chord = RomanNumeral::new(*figure, key.clone())
            .and_then(|numeral| numeral.to_chord())
            .expect("a numeral");
        let mut names: Vec<String> = Vec::new();
        for pitch in chord.pitches() {
            if !names.contains(&pitch.name()) {
                names.push(pitch.name());
            }
        }
        made.add_chord(chord.bass().expect("a bass").clone(), 1.0, names);
    }
    for part in line.overlaid {
        let mut held = Stream::with_kind(StreamKind::Part);
        for (name, length) in *part {
            let duration = Duration::new(*length).expect("a length");
            match name {
                Some(name) => held.push(
                    Note::from_name(name)
                        .expect("a pitch")
                        .with_duration(duration),
                ),
                None => held.push(Rest::new(duration)),
            }
        }
        made.overlay_part(&held);
    }
    made
}

fn our_names(progressions: &[Vec<Vec<Pitch>>]) -> Vec<Vec<Vec<String>>> {
    progressions
        .iter()
        .map(|progression| {
            progression
                .iter()
                .map(|voicing| {
                    voicing
                        .iter()
                        .map(|pitch| music21_name(&pitch.name_with_octave()))
                        .collect()
                })
                .collect()
        })
        .collect()
}

/// The choice `ASKED`'s `Chooser` makes, call by call.
fn chooser() -> impl FnMut(usize) -> usize {
    let mut calls = 0;
    move |count| {
        let index = (calls * 31 + 7) % count;
        calls += 1;
        index
    }
}

#[test]
fn the_realizer_writes_the_scores_music21_s_writes() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        let module = |code: &str, name: &str| -> PyResult<Bound<'_, PyModule>> {
            PyModule::from_code(
                py,
                &std::ffi::CString::new(code).expect("no nul in the helper"),
                &std::ffi::CString::new(format!("{name}.py")).expect("no nul in the name"),
                &std::ffi::CString::new(name).expect("no nul in the name"),
            )
        };
        let helpers = module(STRIP_LAYOUT, "musicxml_parity_helpers")?;
        let asked = module(ASKED, "realizer_asked")?;
        let exporter = py.import("music21.musicxml.m21ToXml")?;
        let today = musicxml_common::pin_encoding_date(py)?;
        let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
        let options = ExportOptions {
            encoding_date: Some(today),
            software: format!("music21 v.{version}"),
            ..ExportOptions::default()
        };
        let written = |score: Bound<'_, PyAny>| -> PyResult<String> {
            let score = helpers.getattr("strip_layout")?.call1((score,))?;
            let general = exporter.getattr("GeneralObjectExporter")?.call1((&score,))?;
            general.setattr("makeNotation", false)?;
            general
                .call_method0("parse")?
                .call_method1("decode", ("utf-8",))?
                .extract::<String>()
        };

        let mut failures = Vec::new();
        let mut compared = 0;
        // What differs between the two documents, if anything does.
        let differs = |what: String,
                       theirs: PyResult<String>,
                       ours: music21_rs::Result<Stream>|
         -> Option<String> {
            let theirs = match theirs {
                Ok(text) => normalize_ids(text.trim_end()),
                Err(error) => {
                    return Some(format!("{what}: music21 could not write it: {error}"));
                }
            };
            let ours = match ours.and_then(|score| to_musicxml(&score, &options)) {
                Ok(text) => normalize_ids(&text),
                Err(error) => {
                    return Some(format!("{what}: the crate could not write it: {error}"));
                }
            };
            let difference = first_difference(&ours, &theirs)?;
            {
                let directory = root.join("target").join("realizer-parity");
                if std::fs::create_dir_all(&directory).is_ok() {
                    let name: String = what
                        .chars()
                        .map(|c| if c.is_alphanumeric() { c } else { '-' })
                        .collect();
                    let _ = std::fs::write(directory.join(format!("{name}.music21.xml")), &theirs);
                    let _ = std::fs::write(directory.join(format!("{name}.music21-rs.xml")), &ours);
                }
                Some(format!("{what}: {difference}"))
            }
        };
        let in_score = |part: music21_rs::Result<Stream>| -> music21_rs::Result<Stream> {
            let mut score = Stream::with_kind(StreamKind::Score);
            score.insert(0.0, part?);
            Ok(score)
        };

        for line in LINES {
            let label = line.label;
            let their_line = || -> PyResult<Bound<'_, PyAny>> {
                asked.getattr("line")?.call1((
                    line.key,
                    line.meter,
                    line.notes.to_vec(),
                    line.chords.to_vec(),
                    line.overlaid
                        .iter()
                        .map(|part| part.to_vec())
                        .collect::<Vec<_>>(),
                ))
            };
            // Each score is asked of a realization made afresh.
            let their_realization = |keyboard: bool| -> PyResult<Bound<'_, PyAny>> {
                let realization = their_line()?.call_method0("realize")?;
                realization.setattr("keyboardStyleOutput", keyboard)?;
                Ok(realization)
            };
            let ours = our_line(line);

            if line.chords.is_empty() {
                compared += 1;
                failures.extend(differs(
                    format!("{label}: the bass line"),
                    their_line()?
                        .call_method0("generateBassLine")
                        .and_then(|part| asked.getattr("in_score")?.call1((part,)))
                        .and_then(written),
                    in_score(ours.generate_bass_line()),
                ));
            } else if their_line()?.call_method0("generateBassLine").is_ok()
                || ours.generate_bass_line().is_ok()
            {
                // Neither writes a bass line of a line of chords.
                failures.push(format!("{label}: a bass line was written of chords"));
            }

            let first = their_realization(true)?;
            let top = Pitch::from_name("B5").expect("a pitch");
            let mut our_realization: Realization = match ours.realize(&Rules::default(), 4, &top) {
                Ok(realization) => realization,
                Err(error) => {
                    failures.push(format!("{label}: the crate could not realize it: {error}"));
                    continue;
                }
            };
            let their_segments: Vec<String> =
                asked.getattr("segments")?.call1((&first,))?.extract()?;
            let our_segments: Vec<String> = our_realization
                .segments()
                .iter()
                .map(|segment| {
                    format!(
                        "{} {:.5}",
                        music21_name(&segment.bass().name_with_octave()),
                        segment.quarter_length()
                    )
                })
                .collect();
            if our_segments != their_segments {
                failures.push(format!(
                    "{label}: the segments differ:\n  music21    {their_segments:?}\n  music21-rs {our_segments:?}"
                ));
                continue;
            }
            let count: usize = first.call_method0("getNumSolutions")?.extract()?;
            if our_realization.num_solutions() != count {
                failures.push(format!(
                    "{label}: music21 finds {count} ways and the crate {}",
                    our_realization.num_solutions()
                ));
                continue;
            }
            let our_progressions = our_realization.all_possibility_progressions();
            let their_names: Vec<Vec<Vec<String>>> = asked
                .getattr("names")?
                .call1((first.call_method0("getAllPossibilityProgressions")?,))?
                .extract()?;
            if our_names(&our_progressions) != their_names {
                failures.push(format!("{label}: the {count} ways differ, or their order"));
                continue;
            }
            println!(
                "{label}: {} segments, {count} ways",
                our_realization.segments().len()
            );

            for keyboard in [true, false] {
                our_realization.set_keyboard_style_output(keyboard);
                let style = if keyboard { "keyboard" } else { "chorale" };
                for index in [0, count / 2, count - 1] {
                    let theirs = their_realization(keyboard)
                        .and_then(|realization| asked.getattr("way")?.call1((realization, index)));
                    compared += 1;
                    failures.extend(differs(
                        format!("{label}: way {index} in {style} style"),
                        theirs.and_then(written),
                        our_realization.generate_realization_from_possibility_progression(
                            &our_progressions[index],
                        ),
                    ));
                }
                compared += 1;
                failures.extend(differs(
                    format!("{label}: a chosen way in {style} style"),
                    their_realization(keyboard)
                        .and_then(|realization| asked.getattr("chosen_way")?.call1((realization,)))
                        .and_then(written),
                    our_realization.generate_random_realization(chooser()),
                ));
                asked.getattr("choose_as_the_crate_does")?.call0()?;
                compared += 1;
                failures.extend(differs(
                    format!("{label}: a way music21 chooses and writes in {style} style"),
                    their_realization(keyboard)
                        .and_then(|realization| realization.call_method0("generateRandomRealization"))
                        .and_then(written),
                    our_realization.generate_random_realization(chooser()),
                ));
                if count <= 12 {
                    compared += 1;
                    failures.extend(differs(
                        format!("{label}: every way in {style} style"),
                        their_realization(keyboard)
                            .and_then(|realization| realization.call_method0("generateAllRealizations"))
                            .and_then(written),
                        our_realization.generate_all_realizations(),
                    ));
                }
                asked.getattr("choose_as_the_crate_does")?.call0()?;
                let kwargs = PyDict::new(py);
                kwargs.set_item("amountToGenerate", 3)?;
                compared += 1;
                failures.extend(differs(
                    format!("{label}: three chosen ways in {style} style"),
                    their_realization(keyboard)
                        .and_then(|realization| {
                            realization.call_method("generateRandomRealizations", (), Some(&kwargs))
                        })
                        .and_then(written),
                    our_realization.generate_random_realizations(3, chooser()),
                ));
            }
        }
        println!("{compared} scores compared");
        Ok((failures, compared))
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    assert!(
        failures.is_empty(),
        "{} of {compared} comparisons differ:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
