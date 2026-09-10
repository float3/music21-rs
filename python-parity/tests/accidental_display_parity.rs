//! Checks `Pitch::update_accidental_display` against what music21's
//! `updateAccidentalDisplay` decides across a grid of situations.
//!
//! `data/accidental_display_expectations.toml` is generated from the
//! music21 submodule by `cargo run --release -p xtask --features python --
//! regenerate-fixtures`, so this test needs neither Python nor the
//! submodule.

use std::path::Path;

use music21_rs::pitch::{Accidental, AccidentalDisplayOptions};
use music21_rs::{KeySignature, Pitch};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Expectations {
    case: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    pitch: String,
    past: Vec<String>,
    past_measure: Vec<String>,
    simultaneous: Vec<String>,
    sharps: i32,
    display_type: Option<String>,
    cautionary_pitch_class: bool,
    cautionary_all: bool,
    override_status: bool,
    cautionary_not_immediate_repeat: bool,
    last_note_was_tied: bool,
    accidental: String,
    shown: Option<bool>,
}

fn expectations() -> Expectations {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|root| root.join("data/accidental_display_expectations.toml"))
        .unwrap_or_default();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    toml::from_str(&text).expect("accidental display expectations parse")
}

fn pitches(names: &[String]) -> Vec<Pitch> {
    names
        .iter()
        .map(|name| Pitch::from_name(name).expect("a pitch name"))
        .collect()
}

#[test]
fn every_accidental_display_decision_matches_music21() {
    let expectations = expectations();
    assert!(expectations.case.len() > 2000, "the grid");

    let mut mismatches = Vec::new();
    for case in &expectations.case {
        let mut pitch = Pitch::from_name(&case.pitch).expect("a pitch name");
        if let Some(display_type) = &case.display_type {
            if pitch.explicit_accidental().is_none() {
                pitch.set_accidental(Some(Accidental::new("natural").expect("a natural")));
            }
            pitch
                .explicit_accidental_mut()
                .expect("an accidental")
                .set_display_type(display_type)
                .expect("a display type");
        }
        let past = pitches(&case.past);
        let past_measure = pitches(&case.past_measure);
        let simultaneous = pitches(&case.simultaneous);
        let altered = KeySignature::new(case.sharps)
            .altered_pitches()
            .expect("altered pitches");
        pitch.update_accidental_display(&AccidentalDisplayOptions {
            pitch_past: &past,
            pitch_past_measure: &past_measure,
            other_simultaneous_pitches: &simultaneous,
            altered_pitches: &altered,
            cautionary_pitch_class: case.cautionary_pitch_class,
            cautionary_all: case.cautionary_all,
            override_status: case.override_status,
            cautionary_not_immediate_repeat: case.cautionary_not_immediate_repeat,
            last_note_was_tied: case.last_note_was_tied,
        });
        let got = match pitch.explicit_accidental() {
            Some(accidental) => (accidental.name().to_string(), accidental.display_status()),
            None => ("none".to_string(), None),
        };
        let want = (case.accidental.clone(), case.shown);
        if got != want {
            mismatches.push(format!("{case:?}:\n    got {got:?}"));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} cases differ from music21:\n{}",
        mismatches.len(),
        expectations.case.len(),
        mismatches.join("\n")
    );
}
