//! The crate's chord reducer against music21's: `analysis.reduceChords`.
//!
//! Each score is written out by music21 as MusicXML with nothing made, and
//! both sides read *that* document -- music21 with `converter.parseData`
//! and the crate with `from_musicxml`, which `musicxml_read_parity` holds
//! to each other -- so the two reducers are given the same score. Each
//! reduces it, and the chords of the two reductions are compared: every
//! measure, and in it every chord and rest with its offset, its length and
//! its pitches.
//!
//! The reduction carries a template besides its chords -- the clef, key,
//! meter, marks and layout music21 copies from the score's first part --
//! which is the readers' business rather than the reducer's and is what
//! `musicxml_read_parity` compares. It is not quite the same on the two
//! sides here: music21's own export writes an empty `<words />` beside the
//! tempo of a mark that says nothing, which music21's reader passes over
//! and the crate's reads as a text expression saying nothing. The chords
//! are what this test is about.
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
from music21 import chord, converter, corpus, note, stream, tree
from music21.analysis import reduceChords
from music21.musicxml import m21ToXml

def written(name):
    score = corpus.parse(name, forceSource=True)
    exporter = m21ToXml.GeneralObjectExporter(score)
    # The score as music21 read it, with nothing made, so that both sides
    # are given the same document to reduce.
    exporter.makeNotation = False
    return exporter.parse().decode('utf-8')

def ties(text):
    """Whether two parts of the score share an instrument id.

    music21 takes the last of `allParts()` as the bass, and `allParts`
    sorts a set of part objects by the id of each part's instrument -- so
    where two share one, as the two staves of a piano do, which part
    music21 calls the bass falls out of the order CPython's set of those
    objects happened to iterate in, which is their addresses in memory.
    Nothing can reproduce that, so such a score is passed over.
    """
    score = converter.parseData(text, format='musicxml', forceSource=True)
    spans = tree.fromStream.asTimespans(score, flatten=True,
                                        classList=(note.Note, chord.Chord))
    ids = [part.getInstrument().partId for part in spans.allParts()]
    return len(set(ids)) != len(ids)

def reduced(text):
    score = converter.parseData(text, format='musicxml', forceSource=True)
    reduction = reduceChords.ChordReducer().run(score, maximumNumberOfChords=3)
    lines = []
    for part in reduction.parts:
        lines.append('Part')
        for measure in part.getElementsByClass(stream.Measure):
            lines.append(f'  Measure {measure.number}')
            for element in measure.getElementsByClass(['NotRest', 'Rest']):
                pitches = ' '.join(p.nameWithOctave for p in getattr(element, 'pitches', ()))
                offset = float(measure.elementOffset(element))
                length = float(element.quarterLength)
                kind = type(element).__name__
                lines.append(f'    {offset:.5f} {length:.5f} {kind} {pitches}')
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

    let (failures, compared, passed_over, refused) =
        Python::attach(|py| -> PyResult<(Vec<String>, usize, usize, usize)> {
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
            let mut passed_over = 0;
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
                // A score whose parts share an instrument id is not comparable:
                // which part music21 calls the bass comes out of the order a
                // set of part objects iterated in.
                if music21
                    .getattr("ties")?
                    .call1((text.as_str(),))?
                    .extract::<bool>()?
                {
                    passed_over += 1;
                    continue;
                }
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
                    Ok(reduction) => chords_of(&reduction),
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
            Ok((failures, compared, passed_over, refused))
        })
        .expect("the comparison runs");

    println!(
        "{compared} compared, {passed_over} passed over for a tie between part ids, {refused} music21 could not write or reduce"
    );
    assert!(
        failures.is_empty(),
        "the two reducers differ:\n  {}",
        failures.join("\n  ")
    );
}

/// The chords and rests of a reduction, measure by measure, as the helper's
/// `reduced` lists music21's.
fn chords_of(reduction: &Stream) -> String {
    let mut lines = String::new();
    for part in reduction.parts() {
        lines.push_str("Part\n");
        for measure in part.measures() {
            lines.push_str(&format!("  Measure {}\n", measure.number()));
            for event in measure.events() {
                let (kind, pitches) = match event.element() {
                    StreamElement::Chord(chord) => (
                        "Chord",
                        chord
                            .pitches()
                            .iter()
                            .map(music21_name)
                            .collect::<Vec<String>>()
                            .join(" "),
                    ),
                    StreamElement::Rest(_) => ("Rest", String::new()),
                    StreamElement::Note(note) => ("Note", music21_name(note.pitch())),
                    _ => continue,
                };
                lines.push_str(&format!(
                    "    {:.5} {:.5} {kind} {pitches}\n",
                    event.offset(),
                    event.element().quarter_length()
                ));
            }
        }
    }
    lines.trim_end().to_string()
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
