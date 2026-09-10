//! Checks what the crate calls a chord against what music21 calls it, for
//! every Forte prime form, a list of spelled chords and chords built from
//! integers, in `data/chord_name_expectations.toml`.

use music21_rs::{Chord, Pitch};

use std::path::Path;

use serde::Deserialize;

fn read<T: serde::de::DeserializeOwned>(fixture: &str) -> T {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|root| root.join(fixture))
        .unwrap_or_default();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    toml::from_str(&text).unwrap_or_else(|err| panic!("{fixture} does not parse: {err}"))
}

fn report(what: &str, total: usize, mismatches: &[String]) {
    assert!(
        mismatches.is_empty(),
        "{} of {total} {what} differ from music21:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[derive(Debug, Deserialize)]
struct Expectations {
    chord: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    forte: Option<String>,
    pitches: Option<String>,
    integers: Option<Vec<i32>>,
    common_name: String,
    pitched_common_name: String,
    quality: String,
    forte_class: String,
    inversion: Option<u8>,
    inversion_name: Option<i32>,
    figure: String,
    root: String,
    bass: String,
}

#[test]
fn every_chord_is_named_the_way_music21_names_it() {
    let expectations: Expectations = read("data/chord_name_expectations.toml");
    assert!(expectations.chord.len() > 400, "the chord set");
    let mut mismatches = Vec::new();
    for case in &expectations.chord {
        let (label, chord) = if let Some(forte) = &case.forte {
            (forte.clone(), Chord::from_forte_class(forte))
        } else if let Some(pitches) = &case.pitches {
            (pitches.clone(), Chord::new(pitches.as_str()))
        } else if let Some(integers) = &case.integers {
            (format!("{integers:?}"), Chord::new(integers.clone()))
        } else {
            panic!("a case names no chord: {case:?}");
        };
        let chord = match chord {
            Ok(chord) => chord,
            Err(err) => {
                mismatches.push(format!("{label}: {err}"));
                continue;
            }
        };
        let got = (
            chord.common_name(),
            chord.pitched_common_name(),
            chord.quality().as_str().to_string(),
            chord.forte_class().unwrap_or_default(),
            chord.inversion(),
            chord.inversion_name().ok().flatten(),
            chord
                .chord_symbol()
                .unwrap_or_else(|| "Chord Symbol Cannot Be Identified".to_string()),
            chord.root().map(Pitch::name).unwrap_or_default(),
            chord.bass().map(Pitch::name).unwrap_or_default(),
        );
        let want = (
            case.common_name.clone(),
            case.pitched_common_name.clone(),
            case.quality.clone(),
            case.forte_class.clone(),
            case.inversion,
            case.inversion_name,
            case.figure.clone(),
            case.root.clone(),
            case.bass.clone(),
        );
        if got != want {
            mismatches.push(format!("{label}:\n    got  {got:?}\n    want {want:?}"));
        }
    }
    report("chords", expectations.chord.len(), &mismatches);
}
