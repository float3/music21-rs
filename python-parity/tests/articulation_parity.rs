//! Checks the articulations transcribed from music21's `articulations`
//! module.
//!
//! Every class's starting values are typed into `src/articulations.rs`; this
//! reads them back through the crate's own API and compares each with what
//! music21 answered, recorded in `data/articulation_expectations.toml` by
//! `cargo run --release -p xtask --features python -- regenerate-fixtures`.

use music21_rs::articulations::{Articulation, ArticulationKind};
use serde::Deserialize;

use std::path::Path;

#[derive(Debug, Deserialize)]
struct Expectations {
    articulation: Vec<Said>,
}

#[derive(Debug, Deserialize)]
struct Said {
    class: String,
    parents: Vec<String>,
    volume_shift: f64,
    length_shift: f64,
    tie_attach: String,
    name: String,
    fields: Vec<String>,
    point_direction: Option<String>,
    harmonic_type: Option<String>,
    number: Option<i32>,
}

fn expectations() -> Expectations {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .join("data/articulation_expectations.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} is unreadable: {err}", path.display()));
    toml::from_str(&text).expect("articulation expectations parse")
}

#[test]
fn every_articulation_starts_out_as_music21_s_class_does() {
    let expected = expectations();
    assert_eq!(
        expected.articulation.len(),
        ArticulationKind::ALL.len(),
        "the classes"
    );
    for said in &expected.articulation {
        let label = &said.class;
        let kind = ArticulationKind::from_class_name(label).expect("a class the crate has");
        let made = Articulation::of_kind(kind);
        let parents: Vec<&str> = kind.parents().to_vec();
        assert_eq!(parents, said.parents, "{label}: parents");
        assert_eq!(
            made.volume_shift(),
            said.volume_shift,
            "{label}: volume shift"
        );
        assert_eq!(
            made.length_shift(),
            said.length_shift,
            "{label}: length shift"
        );
        assert_eq!(made.tie_attach(), said.tie_attach, "{label}: tie attach");
        assert_eq!(made.name(), said.name, "{label}: name");
        for field in &said.fields {
            assert!(kind.has_field(field), "{label}: carries {field}");
        }
        for field in [
            "fingerNumber",
            "substitution",
            "alternate",
            "number",
            "pointDirection",
            "symbol",
            "harmonicType",
            "pitchType",
            "bendAlter",
            "preBend",
            "release",
            "withBar",
        ] {
            assert_eq!(
                kind.has_field(field),
                said.fields.iter().any(|said| said == field),
                "{label}: {field}"
            );
        }
        assert_eq!(
            made.point_direction(),
            said.point_direction.as_deref(),
            "{label}: point"
        );
        assert_eq!(
            made.harmonic_type(),
            said.harmonic_type.as_deref(),
            "{label}: harmonic"
        );
        if let Some(number) = said.number {
            assert_eq!(made.number(), number, "{label}: number");
        }
    }
}
