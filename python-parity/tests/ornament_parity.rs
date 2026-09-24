//! Checks the ornaments ported from music21's `expressions` module.
//!
//! Every class's starting values are typed into `src/expressions.rs`, and
//! what each plays is worked out there; this compares both with what music21
//! answered, recorded in `data/ornament_expectations.toml` by
//! `cargo run --release -p xtask --features python -- regenerate-fixtures`,
//! over a grid of notes, lengths, keys, accidentals, delays and nachschlags.

use music21_rs::expressions::{Ornament, OrnamentDelay, OrnamentKind};
use music21_rs::{Accidental, AccidentalDisplayOptions, Duration, KeySignature, Note, Pitch};
use serde::Deserialize;

use std::path::Path;

#[derive(Debug, Deserialize)]
struct Expectations {
    ornament: Vec<Said>,
    played: Vec<Played>,
}

#[derive(Debug, Deserialize)]
struct Said {
    class: String,
    parents: Vec<String>,
    quarter_length: f64,
    placement: Option<String>,
    tie_attach: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct Played {
    class: String,
    variant: String,
    note: String,
    quarter_length: f64,
    sharps: i32,
    accidental: Option<String>,
    name: String,
    ornamental: Option<Vec<String>>,
    ornamental_error: Option<String>,
    displayed: Option<Vec<String>>,
    before: Option<Vec<String>>,
    main: Option<String>,
    after: Option<Vec<String>>,
    error: Option<String>,
}

fn expectations() -> Expectations {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .join("data/ornament_expectations.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    toml::from_str(&text).expect("ornament expectations parse")
}

fn kind(class: &str) -> OrnamentKind {
    OrnamentKind::from_class_name(class).unwrap_or_else(|| panic!("the crate has no {class}"))
}

/// A played note written as the fixture writes it: `C4 0.25`, with the tie
/// after it where there is one.
fn describe(note: &Note) -> String {
    let length = note.duration().map_or(1.0, Duration::quarter_length);
    let name = note.pitch().name_with_octave();
    match note.tie() {
        Some(tie) => format!("{name} {length:?} {}", tie.tie_type().as_str()),
        None => format!("{name} {length:?}"),
    }
}

#[test]
fn every_ornament_starts_out_as_music21_s_class_does() {
    let expected = expectations();
    assert_eq!(
        expected.ornament.len(),
        OrnamentKind::ALL.len(),
        "the classes"
    );
    for said in &expected.ornament {
        let label = &said.class;
        let kind = kind(label);
        let made = Ornament::of_kind(kind);
        assert_eq!(kind.parents().to_vec(), said.parents, "{label}: parents");
        assert_eq!(
            made.quarter_length(),
            said.quarter_length,
            "{label}: quarter length"
        );
        assert_eq!(
            made.placement(),
            said.placement.as_deref(),
            "{label}: placement"
        );
        assert_eq!(made.tie_attach(), said.tie_attach, "{label}: tie attach");
        assert_eq!(made.name(), said.name, "{label}: name");
    }
}

#[test]
fn every_ornament_plays_what_music21_s_plays() {
    let expected = expectations();
    let mut failures = Vec::new();
    for played in &expected.played {
        let label = format!(
            "{} ({}) on {} lasting {} with {} sharps{}",
            played.class,
            played.variant,
            played.note,
            played.quarter_length,
            played.sharps,
            played
                .accidental
                .as_deref()
                .map(|accidental| format!(", {accidental}"))
                .unwrap_or_default()
        );
        let mut ornament = Ornament::of_kind(kind(&played.class));
        if let Some(accidental) = &played.accidental {
            let (name, shown) = match accidental.strip_suffix(" shown") {
                Some(name) => (name, true),
                None => (accidental.as_str(), false),
            };
            let mut accidental: Accidental = name.parse().expect("an accidental name");
            if shown {
                accidental.set_display_status(Some(true));
            }
            if ornament.is_a("Turn") {
                ornament.set_upper_accidental(Some(accidental));
            } else {
                ornament
                    .set_accidental(Some(accidental))
                    .expect("music21 took the accidental");
            }
        }
        match played.variant.as_str() {
            "plain" => {}
            "nachschlag" => ornament.set_nachschlag(true),
            "default delay" => ornament.set_delay(OrnamentDelay::Default),
            "delay 0.25" => ornament.set_delay(OrnamentDelay::Timed(0.25)),
            other => panic!("an unknown variant {other}"),
        }
        let mut note = Note::from_name(&played.note).expect("a note name");
        note.set_duration(Duration::new(played.quarter_length).expect("a length"));
        let key = KeySignature::new(played.sharps);

        if ornament.name() != played.name {
            failures.push(format!(
                "{label}: name {} not {}",
                ornament.name(),
                played.name
            ));
        }
        let ornamental = ornament
            .ornamental_pitches(note.pitch(), &key)
            .map(|pitches| {
                pitches
                    .iter()
                    .map(|pitch| pitch.name_with_octave())
                    .collect::<Vec<_>>()
            });
        if let Some(theirs) = &played.displayed {
            let past: Vec<Pitch> = ["F#4", "B-3", "C#5"]
                .iter()
                .map(|name| Pitch::from_name(name).expect("a pitch name"))
                .collect();
            let options = AccidentalDisplayOptions {
                pitch_past: &past,
                ..AccidentalDisplayOptions::default()
            };
            let mut pitches = ornament
                .ornamental_pitches(note.pitch(), &key)
                .expect("music21 resolved them");
            ornament.update_accidental_display(&mut pitches, &options);
            let ours: Vec<String> = pitches
                .iter()
                .map(|pitch| match pitch.accidental() {
                    None => "no accidental".to_string(),
                    Some(accidental) => match accidental.display_status() {
                        None => "undecided".to_string(),
                        Some(true) => "shown".to_string(),
                        Some(false) => "hidden".to_string(),
                    },
                })
                .collect();
            if &ours != theirs {
                failures.push(format!("{label}: displayed {ours:?} not {theirs:?}"));
            }
        }
        match (&ornamental, &played.ornamental, &played.ornamental_error) {
            (Ok(ours), Some(theirs), _) if ours != theirs => {
                failures.push(format!("{label}: ornamental {ours:?} not {theirs:?}"));
            }
            (Ok(ours), None, Some(error)) => {
                failures.push(format!(
                    "{label}: ornamental {ours:?} where music21 raised {error}"
                ));
            }
            (Err(error), Some(theirs), _) => {
                failures.push(format!("{label}: ornamental raised {error} not {theirs:?}"));
            }
            _ => {}
        }

        match (ornament.realize(&note, &key), &played.error) {
            (Ok(ours), None) => {
                let before: Vec<String> = ours.before.iter().map(describe).collect();
                let main = ours.main.as_ref().map(describe);
                let after: Vec<String> = ours.after.iter().map(describe).collect();
                let theirs = (
                    played.before.clone().unwrap_or_default(),
                    played.main.clone(),
                    played.after.clone().unwrap_or_default(),
                );
                if (before.clone(), main.clone(), after.clone()) != theirs {
                    failures.push(format!(
                        "{label}: played {before:?} {main:?} {after:?}, music21 {theirs:?}"
                    ));
                }
            }
            (Ok(ours), Some(error)) => failures.push(format!(
                "{label}: played {:?} where music21 raised {error}",
                ours.into_notes().iter().map(describe).collect::<Vec<_>>()
            )),
            (Err(error), None) => {
                failures.push(format!("{label}: raised {error} where music21 played"));
            }
            (Err(_), Some(_)) => {}
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        expected.played.len(),
        failures.join("\n")
    );
}
