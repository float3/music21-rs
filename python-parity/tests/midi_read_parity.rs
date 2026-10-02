//! The crate's MIDI reader against music21's, file by file.
//!
//! Each file is read twice: by music21, which then writes it out as MusicXML
//! with its own exporter, and by `music21_rs::midi::from_midi`, whose score
//! the crate's MusicXML writer writes out. The writer is held to music21's
//! by `musicxml_parity`, so the two documents are the same text only if the
//! two readers made the same score of the file: the same chords and voices,
//! the same measures, ties and rests.
//!
//! music21 reads a MIDI file into lengths no one note value has -- a quarter
//! and a sixteenth -- and its exporter refuses those as they stand. Such a
//! file is compared as an outline instead: every part, measure and voice,
//! and in each the elements in order, with where each stands, how long it
//! lasts, what it sounds and how it is tied.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//!
//! A file is named by its path under the music21 package, or by
//! `corpus:` and a corpus score, which music21 first writes as MIDI.
//! `MIDI_PARITY_FILES`, a `;`-separated list, runs those instead of the ones
//! below.

use music21_rs::midi::from_midi;
use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{OUTLINE, STRIP_LAYOUT, first_difference, normalize_ids, outline};

/// Files both readers read alike, and what each exercises.
const FILES: &[(&str, &str)] = &[
    ("midi/testPrimitive/test01.mid", "one line of notes"),
    ("midi/testPrimitive/test02.mid", "four parts"),
    (
        "midi/testPrimitive/test03.mid",
        "overlapping notes put in voices",
    ),
    ("midi/testPrimitive/test04.mid", "a long ensemble score"),
    ("midi/testPrimitive/test05.mid", "lengths no one value has"),
    ("midi/testPrimitive/test06.mid", "a tune in 6/8"),
    ("midi/testPrimitive/test07.mid", "a long tune with ties"),
    (
        "midi/testPrimitive/test08.mid",
        "an instrument named by the track",
    ),
    ("midi/testPrimitive/test09.mid", "two hands"),
    ("midi/testPrimitive/test10.mid", "rests between notes"),
    (
        "midi/testPrimitive/test11.mid",
        "three parts and a conductor track",
    ),
    ("midi/testPrimitive/test12.mid", "four parts of whole notes"),
    ("midi/testPrimitive/test13.mid", "chords"),
    (
        "midi/testPrimitive/test14.mid",
        "a chord running past a barline",
    ),
    ("midi/testPrimitive/test15.mid", "a change of meter"),
    ("midi/testPrimitive/test16.mid", "a change of tempo"),
    (
        "midi/testPrimitive/test17.mid",
        "three parts with meters changing",
    ),
    ("midi/testPrimitive/test18.mid", "lyrics"),
    ("midi/testPrimitive/test19.mid", "lyrics, one hyphen alone"),
    (
        "midi/testPrimitive/test20.mid",
        "lyrics in another encoding",
    ),
    (
        "midi/testPrimitive/test21.mid",
        "lyrics in another encoding again",
    ),
    ("omr/k525MIDIMvt1.mid", "a string quartet movement"),
    ("omr/k525short.mid", "its opening"),
    (
        "corpus:bach/bwv66.6",
        "a chorale written as MIDI by music21",
    ),
    ("corpus:demos/drum_sample.xml", "a drum part on channel 10"),
    (
        "corpus:schoenberg/opus19/movement6",
        "chords held under moving notes",
    ),
    (
        "corpus:monteverdi/madrigal.3.1.rntxt",
        "chords with lyrics tied across barlines",
    ),
    (
        "corpus:luca/gloria",
        "three voices in triple time, with triplets",
    ),
];

/// The bytes of a corpus score as MIDI.
const HELPERS: &str = r#"
from music21 import corpus, midi

def corpus_midi(name):
    # From the file itself: a cached score is a pickle, and one written
    # while this crate's classes stood in for music21's brings them back.
    score = corpus.parse(name, forceSource=True)
    return midi.translate.streamToMidiFile(score).writestr()
"#;

#[test]
fn the_crate_reads_midi_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, count) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
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
        let strip = module(STRIP_LAYOUT, "musicxml_parity_helpers")?;
        let helpers = module(HELPERS, "midi_parity_helpers")?;
        let outlines = module(OUTLINE, "outline_helpers")?;
        let converter = py.import("music21.converter")?;
        let exporter = py.import("music21.musicxml.m21ToXml")?;
        let today: String = py
            .import("datetime")?
            .getattr("date")?
            .call_method0("today")?
            .str()?
            .extract()?;
        let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
        let software = format!("music21 v.{version}");

        let chosen: Vec<String> = match std::env::var("MIDI_PARITY_FILES") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => FILES.iter().map(|(name, _)| name.to_string()).collect(),
        };
        let mut failures = Vec::new();
        let mut outline_only = 0;
        for name in &chosen {
            let stem = name.replace(['/', '.', ':'], "_");
            let bytes: Vec<u8> = match name.strip_prefix("corpus:") {
                Some(work) => match helpers.getattr("corpus_midi")?.call1((work,)) {
                    Ok(bytes) => bytes.extract()?,
                    Err(error) => {
                        failures.push(format!(
                            "{name}: music21 could not write it as MIDI: {error}"
                        ));
                        continue;
                    }
                },
                None => std::fs::read(root.join("music21").join("music21").join(name))
                    .unwrap_or_else(|error| panic!("reading {name}: {error}")),
            };
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("format", "midi")?;
            let read = match converter.call_method(
                "parseData",
                (pyo3::types::PyBytes::new(py, &bytes),),
                Some(&kwargs),
            ) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not read it: {error}"));
                    continue;
                }
            };
            let ours = from_midi(&bytes);

            // The outline first: it says more plainly what differs.
            let their_outline: String = outlines.getattr("outline")?.call1((&read,))?.extract()?;
            let our_outline = match &ours {
                Ok(score) => outline(score),
                Err(error) => {
                    failures.push(format!("{name}: the crate refused it: {error}"));
                    continue;
                }
            };
            let write = |kind: &str, theirs: &str, ours: &str| {
                let directory = root.join("target").join("midi-read-parity");
                if std::fs::create_dir_all(&directory).is_ok() {
                    let _ =
                        std::fs::write(directory.join(format!("{stem}.music21.{kind}")), theirs);
                    let _ =
                        std::fs::write(directory.join(format!("{stem}.music21-rs.{kind}")), ours);
                }
            };
            if let Some(difference) = first_difference(&our_outline, &their_outline) {
                write("txt", &their_outline, &our_outline);
                failures.push(format!("{name}: outline {difference}"));
                continue;
            }

            let theirs = strip
                .getattr("strip_layout")?
                .call1((read,))
                .and_then(|score| {
                    let general = exporter
                        .getattr("GeneralObjectExporter")?
                        .call1((&score,))?;
                    general.setattr("makeNotation", false)?;
                    general
                        .call_method0("parse")?
                        .call_method1("decode", ("utf-8",))?
                        .extract::<String>()
                });
            // What music21 cannot write as it stands has been compared.
            let Ok(theirs) = theirs else {
                outline_only += 1;
                continue;
            };
            let options = ExportOptions {
                encoding_date: Some(today.clone()),
                software: software.clone(),
                ..ExportOptions::default()
            };
            match ours.and_then(|score| to_musicxml(&score, &options)) {
                Ok(ours) => {
                    let ours = normalize_ids(&ours);
                    let theirs = normalize_ids(theirs.trim_end());
                    if let Some(difference) = first_difference(&ours, &theirs) {
                        write("xml", &theirs, &ours);
                        failures.push(format!("{name}: {difference}"));
                    }
                }
                Err(error) => {
                    failures.push(format!("{name}: the crate could not write it: {error}"))
                }
            }
        }
        println!(
            "{} compared, {outline_only} of them by outline alone",
            chosen.len()
        );
        Ok((failures, chosen.len()))
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    assert!(
        failures.is_empty(),
        "{} of {} files are read differently:\n\n{}",
        failures.len(),
        count,
        failures.join("\n\n")
    );
}
