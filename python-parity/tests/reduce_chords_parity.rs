//! The crate's chord reducer against music21's: `analysis.reduceChords`.
//!
//! Each score is written out by music21 as MusicXML with nothing made, and
//! both sides read *that* document -- music21 with `converter.parseData`
//! and the crate with `from_musicxml`, which `musicxml_read_parity` holds
//! to each other -- so the two reducers are given the same score. Each
//! reduces it, and the two reductions are compared: every measure, and in
//! it every chord and rest with its offset, its length and its pitches,
//! and everything else the reduction carries -- the clef, key, meter,
//! marks and layout music21's template copies from the score's first part
//! -- by its class and where it stands.
//!
//! The template is listed by class and offset alone rather than by what
//! each element says, since what a clef or a mark *says* is the readers'
//! business and `musicxml_read_parity` compares it; which elements the
//! reduction keeps, and where, is the reducer's.
//!
//! A reduction is a score of one part: the chords each measure is made of,
//! at most three a measure, which is music21's `ChordReducer.run`. Every
//! step of the thinning is held to music21 by the comparison of the whole
//! -- a span kept or dropped where music21 did otherwise changes which
//! chords a measure keeps, and by how much.
//!
//! `REDUCE_PARITY_SCORES`, a `;`-separated list of corpus names, runs those
//! instead of the ones below, which is how the corpus is swept.
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::analysis::reduce_chords::{ReductionOptions, reduce_chords};
use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::{Stream, StreamElement};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::first_difference;

/// The scores both reducers reduce alike, and what each exercises.
const SCORES: &[(&str, &str)] = &[
    ("bach/bwv66.6", "a chorale in four parts, with a pickup"),
    ("bach/bwv324", "a chorale whose parts move together"),
    ("monteverdi/madrigal.3.1", "five voices, often in thirds"),
    (
        "luca/gloria",
        "early music in three parts, with meters changing",
    ),
    ("palestrina/agnus_I_01", "four voices in long notes"),
    ("bach/bwv269", "a chorale with a fermata on every phrase"),
    ("bach/bwv1.6", "a chorale with a flowing bass"),
    (
        "mozart/k155/movement1",
        "a quartet whose first violin runs in sixteenths",
    ),
    ("mozart/k156/movement2", "a quartet in a slow three"),
    ("corelli/opus3no1/1grave", "a trio sonata"),
    ("monteverdi/madrigal.5.8", "five voices and a continuo"),
    (
        "beethoven/opus18no1/movement3",
        "a string quartet in three-four",
    ),
    (
        "schoenberg/opus19/movement6",
        "a piano score on two staves, with no key and few notes",
    ),
];

const MUSIC21: &str = r#"
from music21 import converter, corpus, stream
from music21.analysis import reduceChords
from music21.musicxml import m21ToXml

def written(name):
    score = corpus.parse(name, forceSource=True)
    exporter = m21ToXml.GeneralObjectExporter(score)
    # The score as music21 read it, with nothing made, so that both sides
    # are given the same document to reduce.
    exporter.makeNotation = False
    return exporter.parse().decode('utf-8')

def reduced(text):
    score = converter.parseData(text, format='musicxml', forceSource=True)
    reduction = reduceChords.ChordReducer().run(score, maximumNumberOfChords=3)
    lines = []
    for part in reduction.parts:
        lines.append('Part')
        for measure in part.getElementsByClass(stream.Measure):
            lines.append(f'  Measure {measure.number}')
            for element in measure:
                offset = float(measure.elementOffset(element))
                kind = type(element).__name__
                if 'NotRest' in element.classes or 'Rest' in element.classes:
                    pitches = ' '.join(
                        p.nameWithOctave for p in getattr(element, 'pitches', ())
                    )
                    length = float(element.quarterLength)
                    lines.append(f'    {offset:.5f} {length:.5f} {kind} {pitches}')
                else:
                    # What the template carries: its class and where it
                    # stands, not what it says.
                    lines.append(f'    {offset:.5f} {kind}')
    return chr(10).join(lines)
"#;

#[test]
fn the_crate_reduces_a_score_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let chosen: Vec<String> = match std::env::var("REDUCE_PARITY_SCORES") {
        Ok(list) => list
            .split(';')
            .filter(|name| !name.trim().is_empty())
            .map(str::to_string)
            .collect(),
        Err(_) => SCORES.iter().map(|(name, _)| (*name).to_string()).collect(),
    };

    let (failures, compared, refused) =
        Python::attach(|py| -> PyResult<(Vec<String>, usize, usize)> {
            init_py(py)?;
            add_dependency_venv(py, &root)?;
            let _ = py.import("music21")?;
            let music21 = PyModule::from_code(
                py,
                &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
                c"reduce_chords_parity.py",
                c"reduce_chords_parity",
            )?;

            let mut failures = Vec::new();
            let mut compared = 0;
            let mut refused = 0;
            for name in &chosen {
                let text: String = match music21.getattr("written")?.call1((name.as_str(),)) {
                    Ok(text) => text.extract()?,
                    Err(_) => {
                        // A score music21 cannot write as it stands -- a
                        // length no note value has -- leaves the two sides
                        // nothing to read alike.
                        refused += 1;
                        continue;
                    }
                };
                let theirs: String = match music21.getattr("reduced")?.call1((text.as_str(),)) {
                    Ok(listed) => listed.extract()?,
                    Err(error) => {
                        // A score music21's own reducer refuses -- it raises on
                        // a span its steps leave ending before it starts --
                        // leaves nothing to compare.
                        let _ = error;
                        refused += 1;
                        continue;
                    }
                };
                let score = match from_musicxml(&text) {
                    Ok(score) => score,
                    Err(error) => {
                        failures.push(format!("{name}: the crate could not read it: {error}"));
                        continue;
                    }
                };
                let ours = match reduce_chords(&score, &ReductionOptions::default()) {
                    Ok(reduction) => listed(&reduction),
                    Err(error) => {
                        failures.push(format!("{name}: the crate could not reduce it: {error}"));
                        continue;
                    }
                };
                compared += 1;
                if let Some(difference) = first_difference(&ours, &theirs) {
                    write_both(&root, name, &theirs, &ours);
                    failures.push(format!("{name}: {difference}"));
                }
            }
            Ok((failures, compared, refused))
        })
        .expect("the comparison runs");

    println!("{compared} compared, {refused} music21 could not write or reduce");
    assert!(
        failures.is_empty(),
        "the two reducers differ:\n  {}",
        failures.join("\n  ")
    );
}

/// A reduction as the helper's `reduced` lists music21's: its chords and
/// rests with their pitches, and whatever else it carries by class alone.
fn listed(reduction: &Stream) -> String {
    let mut lines = String::new();
    for part in reduction.parts() {
        lines.push_str("Part\n");
        for measure in part.measures() {
            lines.push_str(&format!("  Measure {}\n", measure.number()));
            // The crate keeps a measure's own barlines beside its elements
            // rather than among them, where music21 stands them at the
            // measure's start and end.
            if let Some(barline) = measure.left_barline() {
                lines.push_str(&format!("    {:.5} {}\n", 0.0, named(barline)));
            }
            for event in measure.events() {
                let sounding = match event.element() {
                    StreamElement::Chord(chord) => Some((
                        "Chord",
                        chord
                            .pitches()
                            .iter()
                            .map(music21_name)
                            .collect::<Vec<String>>()
                            .join(" "),
                    )),
                    StreamElement::Rest(_) => Some(("Rest", String::new())),
                    StreamElement::Note(note) => Some(("Note", music21_name(note.pitch()))),
                    _ => None,
                };
                match sounding {
                    Some((kind, pitches)) => lines.push_str(&format!(
                        "    {:.5} {:.5} {kind} {pitches}\n",
                        event.offset(),
                        event.element().quarter_length()
                    )),
                    // What the template carries, named as music21 names its
                    // class.
                    None => lines.push_str(&format!(
                        "    {:.5} {}\n",
                        event.offset(),
                        event
                            .element()
                            .music21_classes()
                            .first()
                            .copied()
                            .unwrap_or("other")
                    )),
                }
            }
            if let Some(barline) = measure.right_barline() {
                lines.push_str(&format!(
                    "    {:.5} {}\n",
                    measure.end_offset(),
                    named(barline)
                ));
            }
        }
    }
    lines.trim_end().to_string()
}

/// A barline as music21 names its class: one that plays a passage again is
/// a `Repeat` there, where the crate keeps the direction on the barline
/// itself.
fn named(barline: &music21_rs::bar::Barline) -> &'static str {
    if barline.repeat_direction().is_some() {
        "Repeat"
    } else {
        "Barline"
    }
}

/// A note as music21 names it: the crate spells a flat `b` where music21
/// spells it `-`, and the step letter may be a `B`.
fn music21_name(pitch: &music21_rs::Pitch) -> String {
    let mut name = pitch.name_with_octave();
    let accidental = name.split_off(1);
    name.push_str(&accidental.replace('b', "-"));
    name
}

/// Both sides of a difference, left where a reader can look at them.
fn write_both(root: &std::path::Path, name: &str, theirs: &str, ours: &str) {
    let directory = root.join("target").join("reduce-chords-parity");
    if std::fs::create_dir_all(&directory).is_ok() {
        let index = name.replace(['/', '.'], "_");
        let _ = std::fs::write(directory.join(format!("{index}.music21.txt")), theirs);
        let _ = std::fs::write(directory.join(format!("{index}.music21-rs.txt")), ours);
    }
}
