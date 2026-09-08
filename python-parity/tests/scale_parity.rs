//! Checks every [`ScaleType`] against what music21 actually produces.
//!
//! The fixture records `getPitches(tonic, tonic + P8)` — the scale over one
//! octave from the tonic — so the comparison here is against
//! [`Scale::pitches_between`] and not [`Scale::pitches`]. The two differ for
//! the plagal modes, whose default realization starts below their final.
//!
//! The expectations in `data/scale_expectations.toml` are generated from the
//! music21 submodule by `cargo run --release -p xtask --features python -- regenerate-fixtures`. They
//! are committed because a full `music21.scale` import needs music21's own
//! dependencies, which the chord-table bridge deliberately stubs out. This test
//! therefore needs neither Python nor the submodule — only the checked-in file.

use music21_rs::{Interval, Pitch, Scale, ScaleType};

/// An octave downwards, for stepping back to the bottom of the range.
fn octave_down() -> Interval {
    Interval::from_name("P8")
        .expect("an octave")
        .reversed()
        .expect("an octave downwards")
}
use serde::Deserialize;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Expectations {
    scale: Vec<ScaleExpectation>,
}

#[derive(Debug, Deserialize)]
struct ScaleExpectation {
    scale_type: String,
    music21_class: String,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    tonic: String,
    pitches: Vec<String>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .to_path_buf()
}

fn expectations() -> Expectations {
    let path = repo_root().join("data/scale_expectations.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    toml::from_str(&text).expect("scale expectations parse")
}

/// Maps the name used in the fixture back to the enum variant.
fn scale_type_for(name: &str) -> ScaleType {
    ScaleType::ALL
        .into_iter()
        .find(|candidate| format!("{candidate:?}") == name)
        .unwrap_or_else(|| panic!("no ScaleType variant named {name}"))
}

#[test]
fn every_scale_matches_music21() {
    let expectations = expectations();
    let mut mismatches = Vec::new();

    for expected in &expectations.scale {
        let scale_type = scale_type_for(&expected.scale_type);
        assert_eq!(
            scale_type.music21_name(),
            expected.music21_class,
            "{} maps to the wrong music21 class",
            expected.scale_type
        );

        for case in &expected.cases {
            let tonic = Pitch::from_name(case.tonic.as_str())
                .unwrap_or_else(|err| panic!("tonic {} is invalid: {err}", case.tonic));
            let top = tonic
                .transpose(&Interval::from_name("P8").expect("an octave"))
                .unwrap_or_else(|err| panic!("octave above {} failed: {err}", case.tonic));
            let actual: Vec<String> = Scale::new(scale_type, tonic)
                .pitches_between(&top.transpose(&octave_down()).expect("octave down"), &top)
                .unwrap_or_else(|err| {
                    panic!("{} on {} failed: {err}", expected.scale_type, case.tonic)
                })
                .iter()
                .map(|pitch| pitch.name_with_octave())
                .collect();

            if actual != case.pitches {
                mismatches.push(format!(
                    "{} on {}\n      music21: {}\n      ours   : {}",
                    expected.scale_type,
                    case.tonic,
                    case.pitches.join(" "),
                    actual.join(" ")
                ));
            }
        }
    }

    assert!(
        mismatches.is_empty(),
        "{} scale realizations differ from music21:\n    {}",
        mismatches.len(),
        mismatches.join("\n    ")
    );
}

#[test]
fn every_scale_type_is_covered_by_the_fixture() {
    let covered: BTreeSet<String> = expectations()
        .scale
        .iter()
        .map(|expected| expected.scale_type.clone())
        .collect();
    let declared: BTreeSet<String> = ScaleType::ALL
        .into_iter()
        .map(|scale_type| format!("{scale_type:?}"))
        .collect();

    assert_eq!(
        declared, covered,
        "every ScaleType needs expectations; regenerate with \
         cargo run --release -p xtask --features python -- regenerate-fixtures"
    );
}
