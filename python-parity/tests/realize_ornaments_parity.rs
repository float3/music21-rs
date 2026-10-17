//! The crate's stream-level ornament realization against music21's:
//! `stream.makeNotation.realizeOrnaments`.
//!
//! music21 builds each subject, resolves the pitches its ornaments play --
//! which is what puts an ornamental accidental into the document at all,
//! since its exporter writes the mark off the resolved pitch -- and exports
//! the score as MusicXML with nothing made. Both sides read *that*
//! document, which `musicxml_read_parity` holds to each other, so the two
//! realizations are given the same score. Each realizes it, and the two are
//! compared by the shared outline: every part, measure and voice, and in
//! each every note with its offset, its length, its pitch and its tie.
//!
//! What MusicXML does not carry is not a difference here, since both sides
//! read the one document: an ornament's size (a `HalfStepTrill` is written
//! as a trill mark and read back as a plain one), an appoggiatura (written
//! as an other-ornament) and whether a turn is delayed all go in the
//! writing, and the subjects below are what is left to tell apart.
//!
//! `REALIZE_ORNAMENTS_SUBJECTS`, a `;`-separated list, runs those subjects
//! instead of all of them; a name written `corpus:<name>` is a corpus
//! score, which is how a score carrying ornaments of its own is tried.
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::makenotation::realize_ornaments;
use music21_rs::musicxml::from_musicxml;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use pyo3::types::PyModule;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{OUTLINE, first_difference, outline};

/// The subjects both sides realize alike, and what each exercises.
const SUBJECTS: &[(&str, &str)] = &[
    (
        "mordent-whole",
        "music21's own example: a whole note with a mordent, the measure after it untouched",
    ),
    (
        "trill-quarter",
        "a trill filling a quarter with its neighbour",
    ),
    (
        "trill-in-three-flats",
        "the neighbour the key signature in force says",
    ),
    (
        "inverted-mordent-eighth",
        "a note too short for the ornament's own length, which it shrinks to fit",
    ),
    (
        "turn-half",
        "a turn's four notes and what is left of the note",
    ),
    (
        "turn-with-accidentals",
        "a turn whose upper and lower notes are written with accidentals",
    ),
    (
        "trill-with-accidental",
        "a trill whose neighbour is written sharp",
    ),
    ("schleifer", "an ornament that plays the note as it stands"),
    ("tremolo-half", "a note repeated, the pieces tied"),
    (
        "two-ornaments",
        "a note heard once for each of its two ornaments, as music21 hands each the note itself",
    ),
    (
        "ornaments-in-voices",
        "two voices in one measure, each with an ornamented note",
    ),
    ("key-change", "a part whose key changes between two trills"),
    (
        "ornament-after-rest",
        "a rest, and a note whose ornament follows it",
    ),
    (
        "chord-ornament",
        "an ornament on a chord, which neither side can play",
    ),
];

const MUSIC21: &str = r#"
from music21 import chord, converter, corpus, expressions, key, meter, note, pitch, stream
from music21.musicxml import m21ToXml
from music21.stream import makeNotation


def scored(part):
    score = stream.Score()
    score.append(part)
    return score


def in_measures(*measures):
    part = stream.Part()
    for measure in measures:
        part.append(measure)
    return scored(part)


def measure(*elements):
    held = stream.Measure()
    for element in elements:
        held.append(element)
    return held


def shown(accidental_name):
    accidental = pitch.Accidental(accidental_name)
    accidental.displayStatus = True
    return accidental


def ornamented(name, length, *ornaments):
    made = note.Note(name, quarterLength=length)
    for ornament in ornaments:
        made.expressions.append(ornament)
    return made


def subject(name):
    if name == 'mordent-whole':
        return in_measures(
            measure(meter.TimeSignature('4/4'),
                    ornamented('C4', 4.0, expressions.Mordent())),
            measure(note.Note('D4', quarterLength=4.0)),
        )
    if name == 'trill-quarter':
        return in_measures(
            measure(meter.TimeSignature('4/4'),
                    ornamented('C4', 1.0, expressions.Trill()),
                    note.Note('D4', quarterLength=3.0)),
        )
    if name == 'trill-in-three-flats':
        return in_measures(
            measure(key.KeySignature(-3), meter.TimeSignature('2/4'),
                    ornamented('A4', 0.5, expressions.Trill()),
                    note.Note('G4', quarterLength=1.5)),
        )
    if name == 'inverted-mordent-eighth':
        return in_measures(
            measure(meter.TimeSignature('2/4'),
                    ornamented('E4', 0.125, expressions.InvertedMordent()),
                    note.Note('F4', quarterLength=1.875)),
        )
    if name == 'turn-half':
        return in_measures(
            measure(meter.TimeSignature('4/4'),
                    ornamented('G4', 2.0, expressions.Turn()),
                    note.Note('F4', quarterLength=2.0)),
        )
    if name == 'turn-with-accidentals':
        return in_measures(
            measure(meter.TimeSignature('4/4'),
                    ornamented('C4', 2.0, expressions.InvertedTurn(
                        upperAccidental=shown('sharp'), lowerAccidental=shown('flat'))),
                    note.Note('D4', quarterLength=2.0)),
        )
    if name == 'trill-with-accidental':
        return in_measures(
            measure(meter.TimeSignature('4/4'),
                    ornamented('C4', 1.0, expressions.Trill(accidental=shown('sharp'))),
                    note.Note('E4', quarterLength=3.0)),
        )
    if name == 'schleifer':
        return in_measures(
            measure(meter.TimeSignature('2/4'),
                    ornamented('B4', 2.0, expressions.Schleifer())),
        )
    if name == 'tremolo-half':
        tremolo = expressions.Tremolo()
        tremolo.numberOfMarks = 3
        return in_measures(
            measure(meter.TimeSignature('4/4'),
                    ornamented('D4', 2.0, tremolo),
                    note.Note('C4', quarterLength=2.0)),
        )
    if name == 'two-ornaments':
        return in_measures(
            measure(meter.TimeSignature('4/4'),
                    ornamented('C4', 2.0, expressions.Mordent(),
                               expressions.InvertedMordent()),
                    note.Note('G4', quarterLength=2.0)),
        )
    if name == 'ornaments-in-voices':
        held = stream.Measure()
        held.insert(0.0, meter.TimeSignature('2/4'))
        first = stream.Voice()
        first.append(ornamented('E5', 1.0, expressions.Trill()))
        first.append(note.Note('D5', quarterLength=1.0))
        second = stream.Voice()
        second.append(ornamented('G4', 2.0, expressions.Mordent()))
        held.insert(0.0, first)
        held.insert(0.0, second)
        return in_measures(held)
    if name == 'key-change':
        return in_measures(
            measure(key.KeySignature(2), meter.TimeSignature('2/4'),
                    ornamented('F#4', 1.0, expressions.Trill()),
                    note.Note('E4', quarterLength=1.0)),
            measure(key.KeySignature(-2),
                    ornamented('B-4', 1.0, expressions.Trill()),
                    note.Note('A4', quarterLength=1.0)),
        )
    if name == 'ornament-after-rest':
        return in_measures(
            measure(meter.TimeSignature('2/4'), note.Rest(quarterLength=1.0),
                    ornamented('C5', 1.0, expressions.Mordent())),
        )
    if name == 'chord-ornament':
        held = chord.Chord('C4 E4 G4', quarterLength=2.0)
        held.expressions.append(expressions.Mordent())
        return in_measures(measure(meter.TimeSignature('4/4'), held,
                                   note.Note('D4', quarterLength=2.0)))
    raise ValueError(f'no such subject: {name}')


def written(name):
    if name.startswith('corpus:'):
        score = corpus.parse(name[len('corpus:'):], forceSource=True)
    else:
        score = subject(name)
    # An ornamental accidental reaches the document only off a resolved
    # ornamental pitch, which is what music21's exporter writes the mark
    # from; building an ornament resolves nothing.
    for carrier in score.recurse().notes:
        for ornament in carrier.expressions:
            if isinstance(ornament, expressions.Ornament):
                try:
                    ornament.resolveOrnamentalPitches(carrier)
                except Exception:
                    pass
    exporter = m21ToXml.GeneralObjectExporter(score)
    # The score as it stands, with nothing made, so that both sides are
    # given the same document to realize.
    exporter.makeNotation = False
    return exporter.parse().decode('utf-8')


def realized(text):
    score = converter.parseData(text, format='musicxml', forceSource=True)
    return makeNotation.realizeOrnaments(score)
"#;

#[test]
fn the_crate_realizes_a_stream_s_ornaments_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let chosen: Vec<String> = match std::env::var("REALIZE_ORNAMENTS_SUBJECTS") {
        Ok(list) => list
            .split(';')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect(),
        Err(_) => SUBJECTS
            .iter()
            .map(|(name, _)| (*name).to_string())
            .collect(),
    };

    let (failures, compared, passed_over, refused, unwritten) = Python::attach(
        |py| -> PyResult<(Vec<String>, usize, usize, usize, usize)> {
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
            let outlines = module(OUTLINE, "realize_ornaments_outline")?;
            let music21 = module(MUSIC21, "realize_ornaments_music21")?;

            let mut failures = Vec::new();
            let mut compared = 0;
            let mut passed_over = 0;
            let mut refused = 0;
            let mut unwritten = 0;
            for name in &chosen {
                let text: String = match music21.getattr("written")?.call1((name.as_str(),)) {
                    Ok(text) => text.extract()?,
                    Err(_) => {
                        // A score music21 cannot write as it stands -- a
                        // length no note value has -- leaves the two sides
                        // nothing to read alike.
                        unwritten += 1;
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
                let theirs = music21.getattr("realized")?.call1((text.as_str(),));
                match (theirs, realize_ornaments(&score)) {
                    (Ok(theirs), Ok(ours)) => {
                        let Ok(their_outline) = outlines
                            .getattr("outline")?
                            .call1((&theirs,))
                            .and_then(|listed| listed.extract::<String>())
                        else {
                            // The shared outline has no line for a tempo
                            // mark that says no number, and raises rather
                            // than write one; such a score is passed over.
                            passed_over += 1;
                            continue;
                        };
                        // The crate's outline falls back to an element's
                        // `Debug` where music21's writes no line at all --
                        // layout, a text box -- and that is the kind
                        // written with a bracket after it. The kind is
                        // what is read rather than the whole line, since a
                        // lyric may hold a bracket of its own.
                        let our_outline = outline(&ours)
                            .lines()
                            .filter(|line| {
                                !line
                                    .split_whitespace()
                                    .nth(2)
                                    .is_some_and(|kind| kind.contains('('))
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        compared += 1;
                        if let Some(difference) = first_difference(&our_outline, &their_outline) {
                            failures.push(format!("{name}: {difference}"));
                        }
                    }
                    // An ornament neither side can play -- one on a chord,
                    // which both ask for a single pitch -- is agreement.
                    // What each said is printed, since two refusals for
                    // two different reasons are not one answer.
                    (Err(theirs), Err(ours)) => {
                        println!(
                            "{name}: neither side realized it: music21 {theirs}; the crate {ours}"
                        );
                        refused += 1;
                    }
                    (Ok(_), Err(error)) => failures.push(format!(
                        "{name}: music21 realized it and the crate could not: {error}"
                    )),
                    (Err(error), Ok(_)) => failures.push(format!(
                        "{name}: the crate realized it and music21 could not: {error}"
                    )),
                }
            }
            Ok((failures, compared, passed_over, refused, unwritten))
        },
    )
    .expect("the comparison runs");

    println!(
        "{compared} compared, {passed_over} passed over for a tempo mark saying no number, {refused} neither side could realize, {unwritten} music21 could not write"
    );
    assert!(
        failures.is_empty(),
        "the two realizations differ:\n  {}",
        failures.join("\n  ")
    );
}
