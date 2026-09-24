//! Checks the clefs transcribed from music21's `clef` module.
//!
//! Every class's starting values are typed into `src/clef.rs`; this reads
//! them back through the crate's own API and compares each with what music21
//! answered, recorded in `data/clef_expectations.toml` by
//! `cargo run --release -p xtask --features python -- regenerate-fixtures`.
//! `clefFromString`, the stem directions and the best clefs are checked
//! behaviourally, over the grids that file records.

use music21_rs::clef::{Clef, ClefKind};
use music21_rs::{Pitch, StemDirection};
use serde::Deserialize;

use std::path::Path;

#[derive(Debug, Deserialize)]
struct Expectations {
    clef: Vec<Said>,
    from_string: Vec<FromString>,
    stem: Vec<Stem>,
    best: Vec<Best>,
}

/// What a clef says about itself.
#[derive(Debug, Deserialize)]
struct Said {
    class: String,
    sign: Option<String>,
    line: Option<u8>,
    octave_change: i32,
    lowest_line: Option<i32>,
    name: String,
    #[serde(default)]
    parents: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FromString {
    text: String,
    octave_shift: i32,
    clef: Option<Said>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Stem {
    class: String,
    pitches: Vec<String>,
    first_last_only: bool,
    extreme_pitch_only: bool,
    direction: String,
}

#[derive(Debug, Deserialize)]
struct Best {
    pitches: Vec<String>,
    allow_treble_8vb: bool,
    class: String,
}

fn expectations() -> Expectations {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .join("data/clef_expectations.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    toml::from_str(&text).expect("clef expectations parse")
}

fn pitches(names: &[String]) -> Vec<Pitch> {
    names
        .iter()
        .map(|name| Pitch::from_name(name).expect("a pitch name"))
        .collect()
}

fn check(made: &Clef, said: &Said, label: &str) {
    assert_eq!(made.kind().class_name(), said.class, "{label}: class");
    assert_eq!(made.sign(), said.sign.as_deref(), "{label}: sign");
    assert_eq!(made.line(), said.line, "{label}: line");
    assert_eq!(
        made.octave_change(),
        said.octave_change,
        "{label}: octave change"
    );
    // music21 has no lowest line at all on a clef that places no pitches.
    if made.places_pitches() || made.is_a("PercussionClef") {
        assert_eq!(made.lowest_line(), said.lowest_line, "{label}: lowest line");
    }
    assert_eq!(made.name(), said.name, "{label}: name");
}

#[test]
fn every_clef_starts_out_as_music21_s_class_does() {
    let expected = expectations();
    assert_eq!(expected.clef.len(), ClefKind::ALL.len(), "the classes");
    for said in &expected.clef {
        let kind = ClefKind::from_class_name(&said.class).expect("a class the crate has");
        check(&Clef::of_kind(kind), said, &said.class);
        let parents: Vec<&str> = kind.parents().to_vec();
        assert_eq!(parents, said.parents, "{}: parents", said.class);
    }
}

#[test]
fn every_clef_string_is_read_as_music21_reads_it() {
    let expected = expectations();
    assert!(expected.from_string.len() > 100);
    for case in &expected.from_string {
        let label = format!("{:?} shifted {}", case.text, case.octave_shift);
        let ours = Clef::from_string(&case.text, case.octave_shift);
        match (&case.clef, ours) {
            (Some(said), Ok(made)) => check(&made, said, &label),
            (None, Err(_)) => assert!(case.error.is_some(), "{label}: an error is recorded"),
            (said, ours) => panic!("{label}: music21 {said:?} {:?}, crate {ours:?}", case.error),
        }
    }
}

#[test]
fn every_stem_goes_the_way_music21_s_does() {
    let expected = expectations();
    for case in &expected.stem {
        let kind = ClefKind::from_class_name(&case.class).expect("a class the crate has");
        let direction = Clef::of_kind(kind)
            .stem_direction_for_pitches(
                &pitches(&case.pitches),
                case.first_last_only,
                case.extreme_pitch_only,
            )
            .expect("pitches to read");
        let written = match direction {
            StemDirection::Down => "down",
            _ => "up",
        };
        assert_eq!(
            written, case.direction,
            "{} over {:?}, first and last {}, extremes {}",
            case.class, case.pitches, case.first_last_only, case.extreme_pitch_only
        );
    }
}

#[test]
fn the_best_clef_is_music21_s() {
    let expected = expectations();
    for case in &expected.best {
        let best = Clef::best_for(&pitches(&case.pitches), case.allow_treble_8vb);
        assert_eq!(
            best.kind().class_name(),
            case.class,
            "{:?} with treble 8vb {}",
            case.pitches,
            case.allow_treble_8vb
        );
    }
}
